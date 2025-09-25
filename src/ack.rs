use rand::Rng;

use crate::{ACK_VALUE, Packet};

#[derive(Debug)]
pub struct ACK {
    content: u8,
}

impl ACK {
    pub const fn new() -> Self {
        Self { content: ACK_VALUE }
    }

    pub fn flip_bit(&mut self, bit_index: u8) {
        if bit_index < 8 {
            self.content ^= 1u8 << bit_index;
        }
    }
}

impl Packet for ACK {
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
        self.content == ACK_VALUE
    }
}
