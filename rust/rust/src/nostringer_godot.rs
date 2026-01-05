use godot::prelude::*;
use nostringer::{sign, verify, SignatureVariant};
use nostringer::blsag::{sign_blsag_hex, verify_blsag_hex};
use nostr::base64::{engine::general_purpose, Engine as _};
use nostringer::types::{BlsagSignature, KeyImage, BlsagSignatureBinary};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Nostringer {
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Nostringer {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl Nostringer {
    #[func]
    pub fn generate_keypair(&self, variant: String) -> VarDictionary {
        let keypair = nostringer::generate_keypair_hex(&variant);
        let mut dict = VarDictionary::new();
        let _ = dict.insert("public_key", keypair.public_key_hex);
        let _ = dict.insert("private_key", keypair.private_key_hex);
        dict
    }

    #[func]
    pub fn sign_bin(&self, message: PackedByteArray, private_key_hex: String, ring_pubkeys: VarArray, variant_str: String) -> VarDictionary {
        let res_text = self.sign(message, private_key_hex, ring_pubkeys, variant_str);
        let mut result = VarDictionary::new();
        let _ = result.insert("signature", PackedByteArray::new());
        let _ = result.insert("key_image", PackedByteArray::new());

        if let Some(sig_v) = res_text.get("signature") {
            let sig_str = sig_v.to_string();
            if sig_str.starts_with("ringA") {
                if let Ok(bytes) = general_purpose::URL_SAFE_NO_PAD.decode(&sig_str[5..]) {
                    let _ = result.insert("signature", PackedByteArray::from_iter(bytes));
                }
            }
        }

        if let Some(ki_v) = res_text.get("key_image") {
            if let Ok(ki_bytes) = hex::decode(ki_v.to_string()) {
                let _ = result.insert("key_image", PackedByteArray::from_iter(ki_bytes));
            }
        }
        result
    }

    #[func]
    pub fn verify_bin(&self, signature: PackedByteArray, message: PackedByteArray, ring_pubkeys: VarArray) -> VarDictionary {
        let mut result = VarDictionary::new();
        let _ = result.insert("valid", false);
        if signature.is_empty() { return result; }
        
        let b64 = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_slice());
        let sig_str = format!("ringA{}", b64);
        
        let is_valid = self.verify_internal(&sig_str, message, ring_pubkeys);
        let _ = result.insert("valid", is_valid);

        if is_valid {
            if let Ok(comp_sig) = nostringer::serialization::CompactSignature::deserialize(&sig_str) {
                 if let nostringer::serialization::CompactSignature::Blsag(_, ki) = comp_sig {
                     if let Ok(ki_bytes) = hex::decode(ki.to_hex()) {
                         let _ = result.insert("key_image", PackedByteArray::from_iter(ki_bytes));
                     }
                 }
            }
        }
        result
    }

    #[func]
    pub fn sign(&self, message: PackedByteArray, private_key_hex: String, ring_pubkeys: VarArray, variant_str: String) -> VarDictionary {
        let ring: Vec<String> = ring_pubkeys.iter_shared().map(|v| v.to_string()).collect();
        let mut result = VarDictionary::new();
        let is_blsag = variant_str.to_lowercase() == "blsag";

        if is_blsag {
             match sign_blsag_hex(message.as_slice(), &private_key_hex, &ring) {
                 Ok((sig, ki)) => {
                     if let Ok(binary_sig) = BlsagSignatureBinary::try_from(&sig) {
                         if let Ok(ki_point) = KeyImage::from_hex(&ki) {
                             let compact = nostringer::serialization::CompactSignature::Blsag(binary_sig, ki_point);
                             if let Ok(sig_compact_str) = compact.serialize() {
                                 let _ = result.insert("signature", sig_compact_str);
                                 let _ = result.insert("key_image", ki);
                             }
                         }
                     }
                 },
                 Err(e) => godot_error!("Nostringer sign BLSAG error: {:?}", e),
             }
        } else {
             match sign(message.as_slice(), &private_key_hex, &ring, SignatureVariant::Sag) {
                 Ok(sig_str) => {
                     let _ = result.insert("signature", sig_str);
                 },
                 Err(e) => godot_error!("Nostringer sign SAG error: {:?}", e),
             }
        }
        result
    }

    #[func]
    pub fn verify(&self, signature: String, message: PackedByteArray, ring_pubkeys: VarArray) -> VarDictionary {
        let mut result = VarDictionary::new();
        let _ = result.insert("valid", false);

        let is_valid = self.verify_internal(&signature, message, ring_pubkeys);
        let _ = result.insert("valid", is_valid);
        
        if is_valid {
            if let Ok(comp_sig) = nostringer::serialization::CompactSignature::deserialize(&signature) {
                if let nostringer::serialization::CompactSignature::Blsag(_, ki) = comp_sig {
                    let _ = result.insert("key_image", ki.to_hex());
                }
            }
        }
        result
    }

    #[func]
    pub fn verify_blsag(&self, signature: String, key_image_hex: String, message: PackedByteArray, ring_pubkeys: VarArray) -> bool {
        let ring: Vec<String> = ring_pubkeys.iter_shared().map(|v| v.to_string()).collect();
        if let Ok(comp_sig) = nostringer::serialization::CompactSignature::deserialize(&signature) {
            if let nostringer::serialization::CompactSignature::Blsag(binary_sig, _ki) = comp_sig {
                let sig_variant = BlsagSignature::from(&binary_sig);
                return verify_blsag_hex(&sig_variant, &key_image_hex, message.as_slice(), &ring).unwrap_or(false);
            }
        }
        false
    }

    #[func]
    pub fn key_images_match(&self, ki1_hex: String, ki2_hex: String) -> bool {
        if let (Ok(ki1), Ok(ki2)) = (
            KeyImage::from_hex(&ki1_hex),
            KeyImage::from_hex(&ki2_hex)
        ) {
            nostringer::blsag::key_images_match(&ki1, &ki2)
        } else {
            false
        }
    }

    fn verify_internal(&self, signature: &str, message: PackedByteArray, ring_pubkeys: VarArray) -> bool {
        let ring: Vec<String> = ring_pubkeys.iter_shared().map(|v| v.to_string()).collect();
        match verify(signature, message.as_slice(), &ring) {
            Ok(v) => v,
            Err(e) => { godot_error!("Nostringer verify error: {:?}", e); false }
        }
    }
}
