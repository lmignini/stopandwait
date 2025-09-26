use rand::Rng;

use crate::{NACK_VALUE, Packet};

#[derive(Debug, PartialEq)]
pub struct NACK {
    content: u8,
}

impl NACK {
    pub const fn new() -> Self {
        Self {
            content: NACK_VALUE,
        }
    }

    pub fn flip_bit(&mut self, bit_index: u8) {
        if bit_index < 8 {
            self.content ^= 1u8 << bit_index;
        }
    }
}

impl Packet for NACK {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        let mut cloned_ack = Self::new();
        for i in 0..8 {
            if rng.random_bool(bit_error_probability) {
                cloned_ack.flip_bit(i);
            }
        }
        cloned_ack
    }
    fn is_valid(&self) -> bool {
        self.content == NACK_VALUE
    }
}
