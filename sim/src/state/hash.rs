use core::hash::{Hash, Hasher};

const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

const PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) fn digest<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut fnv = Fnv(OFFSET_BASIS);
    value.hash(&mut fnv);
    fnv.finish()
}

struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(PRIME);
        }
    }

    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes());
    }

    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes());
    }

    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes());
    }

    fn write_u128(&mut self, i: u128) {
        self.write(&i.to_le_bytes());
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3;

    #[test]
    fn an_empty_byte_slice_digests_to_its_length_prefix_alone() {
        let empty: &[u8] = &[];
        let expected = (0..8).fold(OFFSET_BASIS, |state, _| state.wrapping_mul(PRIME));
        assert_eq!(digest(empty), expected);
        assert_eq!(digest(empty), 0xa8c7_f832_281a_39c5);
    }

    #[test]
    fn the_digest_is_order_sensitive() {
        assert_ne!(digest(&(1u8, 2u8)), digest(&(2u8, 1u8)));
    }

    #[test]
    fn the_same_value_digests_identically_twice() {
        let value = (Vec3::new(1.5, -2.0, 3.25), 7u32, "seven");
        assert_eq!(digest(&value), digest(&value));
    }

    #[test]
    fn a_zero_and_a_negative_zero_digest_differently() {
        assert_ne!(
            digest(&Vec3::new(0.0, 0.0, 0.0)),
            digest(&Vec3::new(-0.0, 0.0, 0.0))
        );
    }

    #[test]
    fn a_usize_digests_as_a_u64_on_every_target() {
        assert_eq!(digest(&0x1234_5678usize), digest(&0x1234_5678u64));
    }

    #[test]
    fn a_slice_digests_as_its_u64_length_then_its_bytes() {
        assert_eq!(digest(&[7u8, 8, 9]), digest(&(3u64, 7u8, 8u8, 9u8)));
    }
}
