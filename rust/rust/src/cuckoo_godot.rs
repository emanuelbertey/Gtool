use godot::prelude::*;
use atomic_cuckoo_filter::CuckooFilter;
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};
use std::fs::File;
use std::io::{Write, Read};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct CuckooFilterGodot {
    filter: Option<CuckooFilter>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for CuckooFilterGodot {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            filter: None,
            base,
        }
    }
}

#[godot_api]
impl CuckooFilterGodot {
    #[func]
    pub fn init_filter(&mut self, capacity: i64, fingerprint_size: i64) {
        let filter = CuckooFilter::builder()
            .capacity(capacity as usize)
            .fingerprint_size(fingerprint_size as usize)
            .build()
            .unwrap();
        self.filter = Some(filter);
    }

    #[func]
    pub fn generate_hash(&self, datos: PackedByteArray) -> i64 {
        let mut hasher = SipHasher13::new_with_keys(0, 0);
        datos.as_slice().hash(&mut hasher);
        hasher.finish() as i64
    }

    #[func]
    pub fn add(&mut self, hash_val: i64) -> bool {
        let val = hash_val as u64;
        if let Some(ref filter) = self.filter {
            // La librería usa 'insert' en lugar de 'add'
            filter.insert(&val).is_ok()
        } else {
            false
        }
    }

    #[func]
    pub fn contains(&self, hash_val: i64) -> bool {
        let val = hash_val as u64;
        if let Some(ref filter) = self.filter {
            filter.contains(&val)
        } else {
            false
        }
    }

    #[func]
    pub fn remove(&mut self, hash_val: i64) -> bool {
        let val = hash_val as u64;
        if let Some(ref filter) = self.filter {
            filter.remove(&val)
        } else {
            false
        }
    }

    #[func]
    pub fn save_to_file(&self, path: String) -> bool {
        if let Some(ref filter) = self.filter {
            // Usamos el método nativo 'to_bytes' de la librería
            let bytes = filter.to_bytes();
            match File::create(path) {
                Ok(mut file) => {
                    file.write_all(&bytes).is_ok()
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    #[func]
    pub fn load_from_file(&mut self, path: String) -> bool {
        match File::open(path) {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                if file.read_to_end(&mut buffer).is_ok() {
                    // Usamos el método nativo 'from_bytes' de la librería
                    match CuckooFilter::from_bytes(&buffer) {
                        Ok(filter) => {
                            self.filter = Some(filter);
                            true
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
}
