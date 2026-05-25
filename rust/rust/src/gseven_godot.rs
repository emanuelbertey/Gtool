use godot::prelude::*;
use sevenz_rust2::{Archive, ArchiveReader, Password, decompress_file, decompress_file_with_password};
use std::fs::File;
use std::path::Path;
use unarc_rs::unified::{ArchiveFormat, ArchiveOptions};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Gseven {
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Gseven {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl Gseven {
    fn entry_to_dict(entry: &sevenz_rust2::ArchiveEntry) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let _ = dict.insert("name", entry.name.clone());
        let _ = dict.insert("size", entry.size as i64);
        let _ = dict.insert("compressed_size", entry.compressed_size as i64);
        let _ = dict.insert("is_directory", entry.is_directory);
        let _ = dict.insert("has_stream", entry.has_stream);
        dict
    }

    fn open_archive(path: &str, password: &str) -> Result<Archive, String> {
        if password.is_empty() {
            Archive::open(path).map_err(|e| format!("{:?}", e))
        } else {
            let pass = Password::new(password);
            Archive::open_with_password(path, &pass).map_err(|e| format!("{:?}", e))
        }
    }

    fn open_reader(path: &str, password: &str) -> Result<ArchiveReader<File>, String> {
        let pass = if password.is_empty() {
            Password::empty()
        } else {
            Password::new(password)
        };
        ArchiveReader::open(path, pass).map_err(|e| format!("{:?}", e))
    }

    #[func]
    pub fn get_entries(&self, path: GString) -> Array<VarDictionary> {
        let p = path.to_string();
        match Self::open_archive(&p, "") {
            Ok(archive) => {
                let mut entries = Array::new();
                for file in &archive.files {
                    entries.push(&Self::entry_to_dict(file));
                }
                entries
            }
            Err(e) => {
                godot_error!("Gseven: {}", e);
                Array::new()
            }
        }
    }

    #[func]
    pub fn get_entries_with_password(&self, path: GString, password: GString) -> Array<VarDictionary> {
        let p = path.to_string();
        let pass = password.to_string();
        match Self::open_archive(&p, &pass) {
            Ok(archive) => {
                let mut entries = Array::new();
                for file in &archive.files {
                    entries.push(&Self::entry_to_dict(file));
                }
                entries
            }
            Err(e) => {
                godot_error!("Gseven: {}", e);
                Array::new()
            }
        }
    }

    #[func]
    pub fn get_entries_multi_volume(&self, paths: Array<GString>) -> Array<VarDictionary> {
        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let opts = ArchiveOptions::new();
        let mut entries = Array::new();
        match ArchiveFormat::open_multi_volume_7z(&path_bufs, opts) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            let mut dict = VarDictionary::new();
                            let name = entry.name().to_string();
                            let size = entry.original_size() as i64;
                            let is_dir = name.ends_with('/') || name.ends_with('\\');
                            let _ = dict.insert("name", name);
                            let _ = dict.insert("size", size);
                            let _ = dict.insert("is_directory", is_dir);
                            entries.push(&dict);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Gseven: error listing multi-volume entry: {:?}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!("Gseven: error opening multi-volume 7z: {:?}", e);
            }
        }
        entries
    }

    #[func]
    pub fn get_entries_multi_volume_with_password(&self, paths: Array<GString>, password: GString) -> Array<VarDictionary> {
        let pass = password.to_string();
        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let opts = if pass.is_empty() {
            ArchiveOptions::new()
        } else {
            ArchiveOptions::new().with_password(&pass)
        };
        let mut entries = Array::new();
        match ArchiveFormat::open_multi_volume_7z(&path_bufs, opts) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            let mut dict = VarDictionary::new();
                            let name = entry.name().to_string();
                            let size = entry.original_size() as i64;
                            let is_dir = name.ends_with('/') || name.ends_with('\\');
                            let _ = dict.insert("name", name);
                            let _ = dict.insert("size", size);
                            let _ = dict.insert("is_directory", is_dir);
                            entries.push(&dict);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Gseven: error listing multi-volume entry: {:?}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!("Gseven: error opening multi-volume 7z with password: {:?}", e);
            }
        }
        entries
    }

    #[func]
    pub fn extract_all(&self, path: GString, dest_dir: GString, password: GString) -> bool {
        let p = path.to_string();
        let dest = dest_dir.to_string();
        let pass = password.to_string();
        let r = if pass.is_empty() {
            decompress_file(&p, &dest)
        } else {
            let pw = Password::new(&pass);
            decompress_file_with_password(&p, &dest, pw)
        };
        match r {
            Ok(_) => true,
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn extract_all_multi_volume(&self, paths: Array<GString>, dest_dir: GString, password: GString) -> bool {
        let dest = dest_dir.to_string();
        let pass = password.to_string();
        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let opts = if pass.is_empty() {
            ArchiveOptions::new()
        } else {
            ArchiveOptions::new().with_password(&pass)
        };
        let out_dir = Path::new(&dest);
        if let Err(e) = std::fs::create_dir_all(out_dir) {
            godot_error!("Gseven: {}", e);
            return false;
        }
        match ArchiveFormat::open_multi_volume_7z(&path_bufs, opts) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            let entry_name = entry.name();
                            let target = out_dir.join(entry_name);
                            if entry_name.ends_with('/') || entry_name.ends_with('\\') {
                                if let Err(e) = std::fs::create_dir_all(&target) {
                                    godot_error!("Gseven: {:?}", e);
                                    return false;
                                }
                            } else {
                                if let Some(parent) = target.parent() {
                                    if let Err(e) = std::fs::create_dir_all(parent) {
                                        godot_error!("Gseven: {:?}", e);
                                        return false;
                                    }
                                }
                                match std::fs::File::create(&target) {
                                    Ok(mut out_file) => {
                                        let read_opts = if pass.is_empty() {
                                            ArchiveOptions::new()
                                        } else {
                                            ArchiveOptions::new().with_password(&pass)
                                        };
                                        if let Err(e) = archive.read_to_with_options(&entry, &mut out_file, &read_opts) {
                                            godot_error!("Gseven: error extracting {}: {:?}", entry_name, e);
                                            return false;
                                        }
                                    }
                                    Err(e) => {
                                        godot_error!("Gseven: {:?}", e);
                                        return false;
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Gseven: {:?}", e);
                            return false;
                        }
                    }
                }
                true
            }
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn extract_entry(&self, path: GString, entry_name: GString, dest_path: GString, password: GString) -> bool {
        let p = path.to_string();
        let target = entry_name.to_string();
        let dest = dest_path.to_string();
        let pass = password.to_string();
        let mut reader = match Self::open_reader(&p, &pass) {
            Ok(r) => r,
            Err(e) => {
                godot_error!("Gseven: {}", e);
                return false;
            }
        };
        let data = match reader.read_file(&target) {
            Ok(d) => d,
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                return false;
            }
        };
        if let Some(parent) = Path::new(&dest).parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                godot_error!("Gseven: {:?}", e);
                return false;
            }
        }
        match std::fs::write(&dest, &data) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn extract_entry_multi_volume(&self, paths: Array<GString>, entry_name: GString, dest_path: GString, password: GString) -> bool {
        let target = entry_name.to_string();
        let dest = dest_path.to_string();
        let pass = password.to_string();
        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let opts = if pass.is_empty() {
            ArchiveOptions::new()
        } else {
            ArchiveOptions::new().with_password(&pass)
        };
        match ArchiveFormat::open_multi_volume_7z(&path_bufs, opts.clone()) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            if entry.name() == target && !entry.name().ends_with('/') {
                                if let Some(parent) = Path::new(&dest).parent() {
                                    if let Err(e) = std::fs::create_dir_all(parent) {
                                        godot_error!("Gseven: {:?}", e);
                                        return false;
                                    }
                                }
                                match std::fs::File::create(&dest) {
                                    Ok(mut out_file) => {
                                        if let Err(e) = archive.read_to_with_options(&entry, &mut out_file, &opts) {
                                            godot_error!("Gseven: {:?}", e);
                                            return false;
                                        }
                                        return true;
                                    }
                                    Err(e) => {
                                        godot_error!("Gseven: {:?}", e);
                                        return false;
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Gseven: {:?}", e);
                            return false;
                        }
                    }
                }
                godot_error!("Gseven: entry not found: {}", target);
                false
            }
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn extract_entry_to_buffer(&self, path: GString, entry_name: GString, password: GString) -> PackedByteArray {
        let p = path.to_string();
        let target = entry_name.to_string();
        let pass = password.to_string();
        let mut reader = match Self::open_reader(&p, &pass) {
            Ok(r) => r,
            Err(e) => {
                godot_error!("Gseven: {}", e);
                return PackedByteArray::new();
            }
        };
        match reader.read_file(&target) {
            Ok(data) => PackedByteArray::from_iter(data.iter().copied()),
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                PackedByteArray::new()
            }
        }
    }

    #[func]
    pub fn extract_entry_multi_volume_to_buffer(&self, paths: Array<GString>, entry_name: GString, password: GString) -> PackedByteArray {
        let target = entry_name.to_string();
        let pass = password.to_string();
        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let opts = if pass.is_empty() {
            ArchiveOptions::new()
        } else {
            ArchiveOptions::new().with_password(&pass)
        };
        match ArchiveFormat::open_multi_volume_7z(&path_bufs, opts.clone()) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            if entry.name() == target && !entry.name().ends_with('/') {
                                match archive.read_with_options(&entry, &opts) {
                                    Ok(data) => return PackedByteArray::from_iter(data),
                                    Err(e) => {
                                        godot_error!("Gseven: {:?}", e);
                                        return PackedByteArray::new();
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Gseven: {:?}", e);
                            return PackedByteArray::new();
                        }
                    }
                }
                godot_error!("Gseven: entry not found: {}", target);
                PackedByteArray::new()
            }
            Err(e) => {
                godot_error!("Gseven: {:?}", e);
                PackedByteArray::new()
            }
        }
    }
}
