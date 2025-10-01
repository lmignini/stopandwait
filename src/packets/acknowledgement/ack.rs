use rand::Rng;

use crate::packets::{ACK_VALUE, Packet, SequenceByte};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ACK {
    content: u16,
}

impl ACK {
    pub const fn new(sequence_byte_of_next_expected_package: SequenceByte) -> Self {
        Self {
            content: u16::from_be_bytes([ACK_VALUE, sequence_byte_of_next_expected_package]),
        }
    }

    // This can build an invalid ACK
    pub fn from_bytes(bytes: [u8; 2]) -> Self {
        Self {
            content: u16::from_be_bytes(bytes),
        }
    }

    pub fn to_bytes(&self) -> [u8; 2] {
        self.content.to_be_bytes()
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

impl Packet for ACK {
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
        self.get_ack_and_sequence_byte().0 == ACK_VALUE
    }
}
