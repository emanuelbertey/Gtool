use godot::prelude::*;

struct RustExtension;

#[gdextension]
unsafe impl ExtensionLibrary for RustExtension {}

//TODO: quitar con confianza si compila



//mod player;
mod peerinfo;
mod state;
mod torrent;
mod keytool;
mod nostrpeer;
mod nostrn;
mod nostr_godot;
mod nostringer_godot;
mod pkarrgodot;
mod shamir_godot;
mod cuckoo_godot;
