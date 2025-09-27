use std::{
    f64,
    fmt::{self, Display},
};

use rand::Rng;

use crate::Packet;

#[derive(Debug, PartialEq, Clone)]
pub struct Frame {
    pub content: Vec<u8>,
}
/*
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
*/
pub fn checksum(msg: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for byte in msg.iter() {
        let mut x = (*byte as u16) ^ (crc & 0xff);
        x ^= (x & 0x0f) << 4;
        crc = ((x << 8) | (crc >> 8)) ^ (x >> 4) ^ (x << 3);
    }
    crc
}

pub fn flip_bit_in_u8(byte: &u8, i: u8) -> u8 {
    byte ^ 1u8 << i
}
impl Frame {
    pub fn new(payload_data: &[u8]) -> Self {
        let mut payload_with_checksum: Vec<u8> = Vec::with_capacity(payload_data.len() + 2);
        for &byte in payload_data {
            payload_with_checksum.push(byte);
        }

        let checksum_in_bytes = checksum(payload_data).to_be_bytes();
        payload_with_checksum.push(checksum_in_bytes[0]);
        payload_with_checksum.push(checksum_in_bytes[1]);
        Self {
            content: payload_with_checksum,
        }
    }
    pub fn get_payload_and_checksum(&self) -> (Vec<u8>, u16) {
        let (payload, checksum) = self
            .content
            .split_last_chunk::<2>()
            .unwrap_or((&[], &[0, 0]));

        (payload.to_vec(), u16::from_be_bytes(*checksum))
    }
}

impl Packet for Frame {
    fn simulate_errors_with_probability(
        &self,
        bit_error_probability: f64,
        rng: &mut rand::rngs::ThreadRng,
    ) -> Self {
        let mut cloned_content = self.content.clone();
        for byte in &mut cloned_content {
            for i in 0..=7 {
                let should_flip = rng.random_bool(bit_error_probability);
                if should_flip {
                    *byte = flip_bit_in_u8(byte, i);
                }
            }
        }
        Self {
            content: cloned_content,
        }
    }
    fn is_valid(&self) -> bool {
        let (received_payload, received_checksum) = self.get_payload_and_checksum();

        let computed_checksum = checksum(&received_payload);

        return received_checksum == computed_checksum;
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res = String::with_capacity(self.content.len() * 9);
        for byte in &self.content {
            res.push_str(&format!("{:08b}", byte));
        }
        write!(f, "{}", res)
    }
}
/*
impl Display for ByteWithParity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Show 8 data bits, then the parity bit
        write!(f, "{:08b}{}", self.byte(), self.parity_bit())
    }
}
    */

#[test]
fn error_detection_encoding_is_applied_correctly() {
    let b1: u8 = 10;
    let b2: u8 = 201;

    let frame = Frame::new(&[b1, b2]);

    println!("{:08b} -> {:08b}", b1, flip_bit_in_u8(&b1, 1));
    println!("{:08b} -> {:08b}", b2, flip_bit_in_u8(&b2, 3));

    println!("{}", frame);
}
