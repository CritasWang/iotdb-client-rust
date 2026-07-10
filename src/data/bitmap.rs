// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Bitmap helpers for the two — deliberately separate — bit conventions in
//! the IoTDB wire protocol (spec gotcha #4):
//!
//! * **Write side** (tablet null bitmaps, §3.3): LSB-first, bit=1 ⇒ null.
//! * **Read side** (TsBlock null indicators and BOOLEAN value arrays, §5.3):
//!   MSB-first, bit=1 ⇒ set (null, or `true` for booleans).
//!
//! Do not mix them.

/// Packs `bits` LSB-first into `ceil(len/8)` bytes: bit `i` → byte `i >> 3`,
/// mask `1 << (i & 7)`. Used for tablet write-side null bitmaps.
pub fn pack_bits_lsb_first(bits: &[bool]) -> Vec<u8> {
    let mut out = vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            out[i >> 3] |= 1 << (i & 7);
        }
    }
    out
}

/// Unpacks `count` bits MSB-first from `bytes`: bit `i` → byte `i >> 3`,
/// mask `0x80 >> (i & 7)`. Used for TsBlock read-side null indicators and
/// BOOLEAN value arrays. Returns `None` if `bytes` is too short.
pub fn unpack_bits_msb_first(bytes: &[u8], count: usize) -> Option<Vec<bool>> {
    if bytes.len() < count.div_ceil(8) {
        return None;
    }
    Some(
        (0..count)
            .map(|i| bytes[i >> 3] & (0x80 >> (i & 7)) != 0)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_lsb_first_bit_positions() {
        // Position 0 → 0x01, position 7 → 0x80.
        assert_eq!(pack_bits_lsb_first(&[true]), vec![0x01]);
        let mut bits = [false; 8];
        bits[7] = true;
        assert_eq!(pack_bits_lsb_first(&bits), vec![0x80]);
    }

    #[test]
    fn pack_lsb_first_multi_byte_and_padding() {
        // 9 bits → 2 bytes; positions 0 and 8 set.
        let mut bits = [false; 9];
        bits[0] = true;
        bits[8] = true;
        assert_eq!(pack_bits_lsb_first(&bits), vec![0x01, 0x01]);
        // Padding bits stay 0.
        assert_eq!(pack_bits_lsb_first(&[false; 3]), vec![0x00]);
        assert_eq!(pack_bits_lsb_first(&[]), Vec::<u8>::new());
    }

    #[test]
    fn unpack_msb_first_bit_positions() {
        // Position 0 → 0x80, position 7 → 0x01.
        assert_eq!(unpack_bits_msb_first(&[0x80], 1), Some(vec![true]));
        let bits = unpack_bits_msb_first(&[0x01], 8).unwrap();
        assert!(bits[7]);
        assert!(bits[..7].iter().all(|&b| !b));
    }

    #[test]
    fn unpack_msb_first_multi_byte() {
        // 0b10100000 0b01000000 over 10 bits → positions 0, 2, 9 set.
        let bits = unpack_bits_msb_first(&[0xA0, 0x40], 10).unwrap();
        let set: Vec<usize> = (0..10).filter(|&i| bits[i]).collect();
        assert_eq!(set, vec![0, 2, 9]);
    }

    #[test]
    fn unpack_msb_first_short_buffer_is_none() {
        assert_eq!(unpack_bits_msb_first(&[0xFF], 9), None);
        assert_eq!(unpack_bits_msb_first(&[], 0), Some(vec![]));
    }

    #[test]
    fn the_two_conventions_differ() {
        // Same logical bit vector, different byte images — guards against
        // accidentally unifying the two implementations.
        let mut bits = [false; 8];
        bits[0] = true;
        let written = pack_bits_lsb_first(&bits); // 0x01
        let read_back = unpack_bits_msb_first(&written, 8).unwrap();
        assert_ne!(read_back, bits.to_vec());
    }
}
