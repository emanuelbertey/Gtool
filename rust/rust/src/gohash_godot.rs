use godot::prelude::*;
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

const HASH_SPACE: i64 = 65_536;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GoHash {
    target_hash: u16,
    has_target: bool,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for GoHash {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            target_hash: 0,
            has_target: false,
            base,
        }
    }
}

#[godot_api]
impl GoHash {
    #[func]
    pub fn generate_hash(&self, datos: PackedByteArray) -> i64 {
        generate_hash64(datos.as_slice(), 0, 0)
    }

    #[func]
    pub fn generate_hash_seeded(&self, datos: PackedByteArray, seed0: i64, seed1: i64) -> i64 {
        generate_hash64(datos.as_slice(), seed0 as u64, seed1 as u64)
    }

    #[func]
    pub fn hash16_from_hash(&self, hash_val: i64) -> i64 {
        fingerprint16(hash_val) as i64
    }

    #[func]
    pub fn set_hash16(&mut self, hash16: i64) {
        self.target_hash = clamp_to_u16(hash16);
        self.has_target = true;
    }

    #[func]
    pub fn set_from_hash(&mut self, hash_val: i64) -> i64 {
        let hash = fingerprint16(hash_val);
        self.target_hash = hash;
        self.has_target = true;
        hash as i64
    }

    #[func]
    pub fn set_from_bytes(&mut self, datos: PackedByteArray) -> i64 {
        self.set_from_hash(generate_hash64(datos.as_slice(), 0, 0))
    }

    #[func]
    pub fn set_from_bytes_seeded(
        &mut self,
        datos: PackedByteArray,
        seed0: i64,
        seed1: i64,
    ) -> i64 {
        self.set_from_hash(generate_hash64(
            datos.as_slice(),
            seed0 as u64,
            seed1 as u64,
        ))
    }

    #[func]
    pub fn set_from_value(&mut self, value: i64) -> i64 {
        let hash = hash16_from_i64_value(value);
        self.target_hash = hash;
        self.has_target = true;
        hash as i64
    }

    #[func]
    pub fn clear(&mut self) {
        self.target_hash = 0;
        self.has_target = false;
    }

    #[func]
    pub fn is_ready(&self) -> bool {
        self.has_target
    }

    #[func]
    pub fn get_hash16(&self) -> i64 {
        self.target_hash as i64
    }

    #[func]
    pub fn storage_bits(&self) -> i64 {
        16
    }

    #[func]
    pub fn storage_bytes(&self) -> i64 {
        2
    }

    #[func]
    pub fn hash_space(&self) -> i64 {
        HASH_SPACE
    }

    #[func]
    pub fn hash16_value(&self, value: i64) -> i64 {
        hash16_from_i64_value(value) as i64
    }

    #[func]
    pub fn contains_hash16(&self, hash16: i64) -> bool {
        self.has_target && self.target_hash == clamp_to_u16(hash16)
    }

    #[func]
    pub fn contains_hash(&self, hash_val: i64) -> bool {
        self.has_target && self.target_hash == fingerprint16(hash_val)
    }

    #[func]
    pub fn contains_bytes(&self, datos: PackedByteArray) -> bool {
        self.contains_hash(generate_hash64(datos.as_slice(), 0, 0))
    }

    #[func]
    pub fn contains_bytes_seeded(&self, datos: PackedByteArray, seed0: i64, seed1: i64) -> bool {
        self.contains_hash(generate_hash64(
            datos.as_slice(),
            seed0 as u64,
            seed1 as u64,
        ))
    }

    #[func]
    pub fn contains_value(&self, value: i64) -> bool {
        self.has_target && self.target_hash == hash16_from_i64_value(value)
    }

    #[func]
    pub fn count_matches(&self, start: i64, end_exclusive: i64) -> i64 {
        if !self.has_target || end_exclusive <= start {
            return 0;
        }

        let mut matches = 0;
        for value in start..end_exclusive {
            if self.target_hash == hash16_from_i64_value(value) {
                matches += 1;
            }
        }
        matches
    }

    #[func]
    pub fn count_false_positives(&self, start: i64, end_exclusive: i64, real_value: i64) -> i64 {
        let matches = self.count_matches(start, end_exclusive);
        if matches == 0 {
            return 0;
        }

        if real_value >= start && real_value < end_exclusive && self.contains_value(real_value) {
            matches - 1
        } else {
            matches
        }
    }

    #[func]
    pub fn expected_matches(&self, total_values: i64) -> f64 {
        total_values.max(0) as f64 / HASH_SPACE as f64
    }
}

fn clamp_to_u16(value: i64) -> u16 {
    (value as u64 & 0xffff) as u16
}

fn generate_hash64(bytes: &[u8], seed0: u64, seed1: u64) -> i64 {
    let mut hasher = SipHasher13::new_with_keys(seed0, seed1);
    bytes.hash(&mut hasher);
    hasher.finish() as i64
}

fn fingerprint16(hash_val: i64) -> u16 {
    (hash_val as u64 & 0xffff) as u16
}

fn hash16_from_i64_value(value: i64) -> u16 {
    fingerprint16(generate_hash64(&value.to_le_bytes(), 0, 0))
}

#[cfg(test)]
mod tests {
    use super::{fingerprint16, generate_hash64, hash16_from_i64_value};

    #[test]
    fn generated_hash_is_reduced_to_16_bits() {
        let hash = generate_hash64(b"dato_prueba", 0, 0);
        let fp = fingerprint16(hash);

        assert_eq!(fp as i64, hash & 0xffff);
    }

    #[test]
    fn seeded_hash_changes_output() {
        let default_hash = generate_hash64(b"dato_prueba", 0, 0);
        let seeded_hash = generate_hash64(b"dato_prueba", 123, 456);

        assert_ne!(default_hash, seeded_hash);
    }

    #[test]
    fn sixteen_million_values_are_close_to_256_candidates_per_hash() {
        let target = hash16_from_i64_value(1_234_567);
        let matches = (0..16_777_216)
            .filter(|value| hash16_from_i64_value(*value) == target)
            .count();

        assert!((192..=320).contains(&matches));
    }
}
