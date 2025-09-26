use std::{
    f64,
    fmt::{self, Display},
};

use rand::Rng;

use crate::Packet;

#[derive(Debug, PartialEq, Clone)]
pub struct Frame {
    pub content: Vec<ByteWithParity>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ByteWithParity {
    // bit 0: parity bit, bits 1..=8: data byte
    packed: u16,
}

impl ByteWithParity {
    fn new(byte: u8) -> Self {
        let parity_bit: u16 = (byte.count_ones() % 2 != 0) as u16;
        Self {
            // data first (bits 1..=8), parity last (bit 0)
            packed: ((byte as u16) << 1) | parity_bit,
        }
    }

    pub fn byte(&self) -> u8 {
        ((self.packed >> 1) & 0xFF) as u8
    }

    pub fn parity_bit(&self) -> u8 {
        (self.packed & 1) as u8
    }

    pub fn set_parity_bit(&mut self, bit: u8) {
        self.packed = (self.packed & !1) | ((bit & 1) as u16);
    }

    pub fn flip_bit(&mut self, bit_index: u8) {
        // bit_index: 0..=7 for data bits (LSB..MSB), 8 for parity bit (last)
        if bit_index <= 7 {
            self.packed ^= 1u16 << (bit_index + 1);
        } else if bit_index == 8 {
            self.packed ^= 1u16; // parity at bit 0
        }
    }

    pub fn is_valid(&self) -> bool {
        (self.byte().count_ones() + self.parity_bit() as u32) % 2 == 0
    }
}

impl Frame {
    pub fn new(payload_data: &[u8]) -> Self {
        let mut payload_with_parities: Vec<ByteWithParity> = Vec::with_capacity(payload_data.len());
        for &byte in payload_data {
            payload_with_parities.push(ByteWithParity::new(byte));
        }
        Self {
            content: payload_with_parities,
        }
    }

    pub fn get_original_payload(&self) -> Vec<u8> {
        let mut original_payload = Vec::with_capacity(self.content.len());
        for byte_with_parity in &self.content {
            original_payload.push(byte_with_parity.byte());
        }
        original_payload
    }
}

impl Packet for Frame {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        let mut cloned_content = self.content.clone();
        for byte_with_parity in &mut cloned_content {
            for i in 0..=8 {
                let should_flip = rng.random_bool(bit_error_probability);
                if should_flip {
                    byte_with_parity.flip_bit(i);
                }
            }
        }
        Self {
            content: cloned_content,
        }
    }
    fn is_valid(&self) -> bool {
        self.content
            .iter()
            .all(|byte_with_parity| byte_with_parity.is_valid())
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res = String::with_capacity(self.content.len() * 9);
        for bwp in &self.content {
            res.push_str(&bwp.to_string());
        }
        write!(f, "{}", res)
    }
}

impl Display for ByteWithParity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Show 8 data bits, then the parity bit
        write!(f, "{:08b}{}", self.byte(), self.parity_bit())
    }
}
