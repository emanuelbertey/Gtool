use godot::prelude::*;
use shamirsecretsharing::*;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Shamir {
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Shamir {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl Shamir {
    #[func]
    pub fn create_shares(&self, data: PackedByteArray, count: i32, threshold: i32) -> Variant {
        let data_slice = data.as_slice();
        match create_shares(data_slice, count as u8, threshold as u8) {
            Ok(shares) => {
                let mut godot_shares = VarArray::new();
                for share in shares {
                    godot_shares.push(&PackedByteArray::from_iter(share).to_variant());
                }
                godot_shares.to_variant()
            }
            Err(e) => {
                godot_error!("Shamir create_shares error: {:?}", e);
                Variant::nil()
            }
        }
    }

    #[func]
    pub fn combine_shares(&self, shares: VarArray) -> Variant {
        let mut rust_shares = Vec::new();
        for share in shares.iter_shared() {
            if let Ok(pba) = share.try_to::<PackedByteArray>() {
                rust_shares.push(pba.to_vec());
            } else {
                godot_error!("Shamir combine_shares: invalid share type, expected PackedByteArray");
                return Variant::nil();
            }
        }

        match combine_shares(&rust_shares) {
            Ok(Some(restored)) => {
                PackedByteArray::from_iter(restored).to_variant()
            }
            Ok(None) => {
                // Secret lost (not enough shares)
                Variant::nil()
            }
            Err(e) => {
                godot_error!("Shamir combine_shares error: {:?}", e);
                Variant::nil()
            }
        }
    }
}
