use godot::prelude::*;
use log::{Level, Metadata, Record};

struct GodotLogger;

impl log::Log for GodotLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!("[{}] {}", record.level(), record.args());
            if record.level() <= Level::Warn {
                godot_warn!("{}", msg);
            } else {
                godot_print!("{}", msg);
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: GodotLogger = GodotLogger;

struct RustExtension;

#[gdextension]
unsafe impl ExtensionLibrary for RustExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Core {
            let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
            // In rustls 0.23, you must manually install the default crypto provider 
            // if both 'aws-lc-rs' and 'ring' are enabled or if no default is chosen via features.
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    }
}

//TODO: quitar con confianza si compila



mod ntny;
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
mod infotorrent_godot;
mod xlstm_large_node;