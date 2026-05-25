use godot::prelude::*;
use rars::{Archive, ArchiveReader, ArchiveReadOptions, extract_volumes_to, ExtractedEntryMeta};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

struct CaptureWriter {
    buf: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Write for CaptureWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if let Ok(mut guard) = self.buf.lock() {
            if let Some(ref mut buf) = *guard {
                buf.extend_from_slice(data);
            }
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn make_pass(pass: &GString) -> Option<Vec<u8>> {
    let s = pass.to_string();
    if s.is_empty() { None } else { Some(s.into_bytes()) }
}

fn open_archive(path: &str, password: Option<&[u8]>) -> Result<Archive, String> {
    match password {
        Some(pass) => ArchiveReader::read_path_with_options(path, ArchiveReadOptions::with_password(pass))
            .map_err(|e| format!("{:?}", e)),
        None => ArchiveReader::read_path(path).map_err(|e| format!("{:?}", e)),
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Grrar {
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Grrar {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl Grrar {
    fn entry_to_dict(meta: &rars::ArchiveMemberMeta) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let _ = dict.insert("name", String::from_utf8_lossy(&meta.name).to_string());
        let _ = dict.insert("size", meta.unpacked_size as i64);
        let _ = dict.insert("packed_size", meta.packed_size as i64);
        let _ = dict.insert("is_directory", meta.is_directory);
        let _ = dict.insert("is_encrypted", meta.is_encrypted);
        let _ = dict.insert("is_stored", meta.is_stored);
        let _ = dict.insert("is_split", meta.is_split_before || meta.is_split_after);
        dict
    }

    fn list_entries(archive: &Archive) -> Array<VarDictionary> {
        let mut entries = Array::new();
        for member in archive.members() {
            entries.push(&Self::entry_to_dict(&member.meta));
        }
        entries
    }

    #[func]
    pub fn get_entries(&self, path: GString) -> Array<VarDictionary> {
        let p = path.to_string();
        match open_archive(&p, None) {
            Ok(a) => Self::list_entries(&a),
            Err(e) => {
                godot_error!("Grrar: {}", e);
                Array::new()
            }
        }
    }

    #[func]
    pub fn get_entries_with_password(&self, path: GString, password: GString) -> Array<VarDictionary> {
        let p = path.to_string();
        let pass = make_pass(&password);
        match open_archive(&p, pass.as_deref()) {
            Ok(a) => Self::list_entries(&a),
            Err(e) => {
                godot_error!("Grrar: {}", e);
                Array::new()
            }
        }
    }

    #[func]
    pub fn get_entries_multi_volume(&self, paths: Array<GString>) -> Array<VarDictionary> {
        match open_volumes(&paths, None) {
            Ok(v) if !v.is_empty() => Self::list_entries(&v[0]),
            _ => Array::new(),
        }
    }

    #[func]
    pub fn get_entries_multi_volume_with_password(&self, paths: Array<GString>, password: GString) -> Array<VarDictionary> {
        let pass = make_pass(&password);
        match open_volumes(&paths, pass.as_deref()) {
            Ok(v) if !v.is_empty() => Self::list_entries(&v[0]),
            _ => Array::new(),
        }
    }

    #[func]
    pub fn extract_all(&self, path: GString, dest_dir: GString, password: GString) -> bool {
        let p = path.to_string();
        let dest = dest_dir.to_string();
        let pass = make_pass(&password);
        let archive = match open_archive(&p, pass.as_deref()) {
            Ok(a) => a,
            Err(e) => {
                godot_error!("Grrar: {}", e);
                return false;
            }
        };
        extract_ok(archive.extract_to(pass.as_deref(), |meta| {
            let name = String::from_utf8_lossy(&meta.name);
            let full = Path::new(&dest).join(name.as_ref());
            if meta.is_directory {
                let _ = std::fs::create_dir_all(&full);
                Ok(Box::new(io::sink()) as Box<dyn Write>)
            } else {
                if let Some(parent) = full.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let file = std::fs::File::create(&full)?;
                Ok(Box::new(file) as Box<dyn Write>)
            }
        }))
    }

    #[func]
    pub fn extract_all_multi_volume(&self, paths: Array<GString>, dest_dir: GString, password: GString) -> bool {
        let dest = dest_dir.to_string();
        let pass = make_pass(&password);
        let archives = match open_volumes(&paths, pass.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("Grrar: {}", e);
                return false;
            }
        };
        extract_ok(extract_volumes_to(&archives, pass.as_deref(), |meta| {
            let name = String::from_utf8_lossy(&meta.name);
            let full = Path::new(&dest).join(name.as_ref());
            if meta.is_directory {
                let _ = std::fs::create_dir_all(&full);
                Ok(Box::new(io::sink()) as Box<dyn Write>)
            } else {
                if let Some(parent) = full.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let file = std::fs::File::create(&full)?;
                Ok(Box::new(file) as Box<dyn Write>)
            }
        }))
    }

    #[func]
    pub fn extract_entry(&self, path: GString, entry_name: GString, dest_path: GString, password: GString) -> bool {
        let p = path.to_string();
        let target = entry_name.to_string();
        let dest = dest_path.to_string();
        let pass = make_pass(&password);
        let archive = match open_archive(&p, pass.as_deref()) {
            Ok(a) => a,
            Err(e) => {
                godot_error!("Grrar: {}", e);
                return false;
            }
        };
        extract_ok(archive.extract_to(pass.as_deref(), |meta| {
            let name = String::from_utf8_lossy(&meta.name).to_string();
            if name == target && !meta.is_directory {
                if let Some(parent) = Path::new(&dest).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let file = std::fs::File::create(&dest)?;
                return Ok(Box::new(file) as Box<dyn Write>);
            }
            Ok(Box::new(io::sink()) as Box<dyn Write>)
        }))
    }

    #[func]
    pub fn extract_entry_multi_volume(&self, paths: Array<GString>, entry_name: GString, dest_path: GString, password: GString) -> bool {
        let target = entry_name.to_string();
        let dest = dest_path.to_string();
        let pass = make_pass(&password);
        let archives = match open_volumes(&paths, pass.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("Grrar: {}", e);
                return false;
            }
        };
        if archives.is_empty() {
            return false;
        }
        extract_ok(extract_volumes_to(&archives, pass.as_deref(), |meta| {
            let name = String::from_utf8_lossy(&meta.name).to_string();
            if name == target && !meta.is_directory {
                if let Some(parent) = Path::new(&dest).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let file = std::fs::File::create(&dest)?;
                return Ok(Box::new(file) as Box<dyn Write>);
            }
            Ok(Box::new(io::sink()) as Box<dyn Write>)
        }))
    }

    #[func]
    pub fn extract_entry_to_buffer(&self, path: GString, entry_name: GString, password: GString) -> PackedByteArray {
        let p = path.to_string();
        let target = entry_name.to_string();
        let pass = make_pass(&password);
        let archive = match open_archive(&p, pass.as_deref()) {
            Ok(a) => a,
            Err(e) => {
                godot_error!("Grrar: {}", e);
                return PackedByteArray::new();
            }
        };
        extract_one_to_buf(&archive, pass.as_deref(), &target)
    }

    #[func]
    pub fn extract_entry_multi_volume_to_buffer(&self, paths: Array<GString>, entry_name: GString, password: GString) -> PackedByteArray {
        let target = entry_name.to_string();
        let pass = make_pass(&password);
        let archives = match open_volumes(&paths, pass.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("Grrar: {}", e);
                return PackedByteArray::new();
            }
        };
        if archives.is_empty() {
            return PackedByteArray::new();
        }
        let captured = Arc::new(Mutex::new(None::<Vec<u8>>));
        let cb = captured.clone();
        let tgt = target.clone();
        let result = extract_volumes_to(&archives, pass.as_deref(), |meta| {
            let name = String::from_utf8_lossy(&meta.name).to_string();
            if name == tgt && !meta.is_directory {
                let mut guard = cb.lock().unwrap();
                *guard = Some(Vec::new());
                return Ok(Box::new(CaptureWriter { buf: cb.clone() }) as Box<dyn Write>);
            }
            Ok(Box::new(io::sink()) as Box<dyn Write>)
        });
        take_buf(captured, result)
    }
}

fn open_volumes(paths: &Array<GString>, password: Option<&[u8]>) -> Result<Vec<Archive>, String> {
    let vec: Vec<String> = paths.iter_shared().map(|p| p.to_string()).collect();
    if vec.is_empty() {
        return Err("No paths".to_string());
    }
    vec.iter().map(|p| open_archive(p, password)).collect()
}

fn extract_one_to_buf(archive: &Archive, password: Option<&[u8]>, target: &str) -> PackedByteArray {
    let captured = Arc::new(Mutex::new(None::<Vec<u8>>));
    let cb = captured.clone();
    let tgt = target.to_string();
    let result = archive.extract_to(password, |meta| {
        let name = String::from_utf8_lossy(&meta.name).to_string();
        if name == tgt && !meta.is_directory {
            let mut guard = cb.lock().unwrap();
            *guard = Some(Vec::new());
            return Ok(Box::new(CaptureWriter { buf: cb.clone() }) as Box<dyn Write>);
        }
        Ok(Box::new(io::sink()) as Box<dyn Write>)
    });
    take_buf(captured, result)
}

fn extract_ok(result: Result<(), rars::Error>) -> bool {
    match result {
        Ok(_) => true,
        Err(e) => {
            godot_error!("Grrar: {:?}", e);
            false
        }
    }
}

fn take_buf(captured: Arc<Mutex<Option<Vec<u8>>>>, result: Result<(), rars::Error>) -> PackedByteArray {
    match result {
        Ok(_) => {
            let guard = captured.lock().unwrap();
            if let Some(ref data) = *guard {
                PackedByteArray::from_iter(data.iter().copied())
            } else {
                godot_error!("Grrar: entry not found or is a directory");
                PackedByteArray::new()
            }
        }
        Err(e) => {
            godot_error!("Grrar: {:?}", e);
            PackedByteArray::new()
        }
    }
}
