use godot::prelude::*;
use unarc_rs::unified::{ArchiveFormat, is_supported_archive};
use std::path::Path;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Unarc {
    base: Base<RefCounted>,
    #[export]
    entries_list: Array<VarDictionary>,
}

#[godot_api]
impl IRefCounted for Unarc {
    fn init(base: Base<RefCounted>) -> Self {
        Self { 
            base,
            entries_list: Array::new(),
        }
    }
}

#[godot_api]
impl Unarc {
    // Retorna una lista con la información (nombre, tamaño, si es directorio) de todas las entradas del archivo comprimido
    #[func]
    pub fn get_entries(&mut self, archive_path: String) -> Array<VarDictionary> {
        let mut entries_array = Array::new();
        let path = Path::new(&archive_path);
        
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return entries_array;
        }

        match ArchiveFormat::open_path(path) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            let mut dict = VarDictionary::new();
                            let name = entry.name().to_string();
                            let size = entry.original_size() as i64;
                            let is_dir = name.ends_with('/') || name.ends_with('\\');
                            
                            dict.insert("name", name);
                            dict.insert("size", size);
                            dict.insert("is_directory", is_dir);
                            entries_array.push(&dict);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Error leyendo la entrada del archivo comprimido: {:?}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo comprimido {}: {:?}", archive_path, e);
            }
        }

        self.entries_list = entries_array.clone();
        entries_array
    }

    // Extrae todo el archivo comprimido a una carpeta destino (sin contraseña)
    #[func]
    pub fn extract_all(&self, archive_path: String, output_dir: String) -> bool {
        self.extract_all_with_password(archive_path, output_dir, String::new())
    }

    // Extrae todo el archivo comprimido a una carpeta destino (soporta contraseña opcional)
    #[func]
    pub fn extract_all_with_password(&self, archive_path: String, output_dir: String, password: String) -> bool {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return false;
        }

        let out_dir = Path::new(&output_dir);
        if let Err(e) = std::fs::create_dir_all(out_dir) {
            godot_error!("No se pudo crear el directorio de salida {}: {:?}", output_dir, e);
            return false;
        }

        let format = match ArchiveFormat::from_path(path) {
            Some(fmt) => fmt,
            None => {
                godot_error!("No se pudo determinar el formato del archivo para extracción: {}", archive_path);
                return false;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    let entry_name = entry.name();
                                    let target_path = out_dir.join(entry_name);

                                    if entry_name.ends_with('/') || entry_name.ends_with('\\') {
                                        if let Err(e) = std::fs::create_dir_all(&target_path) {
                                            godot_error!("No se pudo crear el directorio {:?}: {:?}", target_path, e);
                                            return false;
                                        }
                                    } else {
                                        if let Some(parent) = target_path.parent() {
                                            if let Err(e) = std::fs::create_dir_all(parent) {
                                                godot_error!("No se pudo crear el directorio padre {:?}: {:?}", parent, e);
                                                return false;
                                            }
                                        }
                                        match std::fs::File::create(&target_path) {
                                            Ok(mut out_file) => {
                                                let res = if password.is_empty() {
                                                    archive.read_to(&entry, &mut out_file)
                                                } else {
                                                    let read_opts = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                                    archive.read_to_with_options(&entry, &mut out_file, &read_opts)
                                                };

                                                if let Err(e) = res {
                                                    godot_error!("Error escribiendo flujo al archivo {:?}: {:?}", target_path, e);
                                                    return false;
                                                }
                                            }
                                            Err(e) => {
                                                godot_error!("No se pudo crear el archivo {:?}: {:?}", target_path, e);
                                                return false;
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error iterando entradas: {:?}", e);
                                    return false;
                                }
                            }
                        }
                        true
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {}: {:?}", archive_path, e);
                        false
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {}: {:?}", archive_path, e);
                false
            }
        }
    }

    // Extrae una sola entrada del archivo a una ruta específica (sin contraseña)
    #[func]
    pub fn extract_entry(&self, archive_path: String, entry_name: String, dest_path: String) -> bool {
        self.extract_entry_with_password(archive_path, entry_name, dest_path, String::new())
    }

    // Extrae una sola entrada del archivo a una ruta específica (soporta contraseña opcional)
    #[func]
    pub fn extract_entry_with_password(&self, archive_path: String, entry_name: String, dest_path: String, password: String) -> bool {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return false;
        }

        let format = match ArchiveFormat::from_path(path) {
            Some(fmt) => fmt,
            None => {
                godot_error!("No se pudo determinar el formato del archivo para extracción individual: {}", archive_path);
                return false;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    if entry.name() == entry_name {
                                        if let Some(parent) = Path::new(&dest_path).parent() {
                                            if let Err(e) = std::fs::create_dir_all(parent) {
                                                godot_error!("No se pudo crear el directorio {:?}: {:?}", parent, e);
                                                return false;
                                            }
                                        }
                                        match std::fs::File::create(&dest_path) {
                                            Ok(mut out_file) => {
                                                let res = if password.is_empty() {
                                                    archive.read_to(&entry, &mut out_file)
                                                } else {
                                                    let read_opts = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                                    archive.read_to_with_options(&entry, &mut out_file, &read_opts)
                                                };

                                                if let Err(e) = res {
                                                    godot_error!("Error escribiendo flujo al archivo {}: {:?}", dest_path, e);
                                                    return false;
                                                }
                                                return true;
                                            }
                                            Err(e) => {
                                                godot_error!("No se pudo crear el archivo {}: {:?}", dest_path, e);
                                                return false;
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error iterando entradas: {:?}", e);
                                    break;
                                }
                            }
                        }
                        godot_warn!("Entrada no encontrada en el archivo: {}", entry_name);
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {}: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {}: {:?}", archive_path, e);
            }
        }
        false
    }

    // Retorna los bytes sin comprimir de una sola entrada en el archivo
    #[func]
    pub fn read_entry_bytes(&self, archive_path: String, entry_name: String) -> PackedByteArray {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return PackedByteArray::new();
        }

        match ArchiveFormat::open_path(path) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            if entry.name() == entry_name {
                                match archive.read(&entry) {
                                    Ok(data) => {
                                        return PackedByteArray::from_iter(data);
                                    }
                                    Err(e) => {
                                        godot_error!("Error leyendo la entrada {}: {:?}", entry_name, e);
                                        return PackedByteArray::new();
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Error iterando entradas: {:?}", e);
                            break;
                        }
                    }
                }
                godot_warn!("Entrada no encontrada en el archivo: {}", entry_name);
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo comprimido: {:?}", e);
            }
        }
        PackedByteArray::new()
    }

    #[func]
    pub fn get_entries_with_password(&mut self, archive_path: String, password: String) -> Array<VarDictionary> {
        let mut entries_array = Array::new();
        let path = Path::new(&archive_path);
        
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return entries_array;
        }

        let format = match ArchiveFormat::from_path(path) {
            Some(fmt) => fmt,
            None => {
                godot_error!("No se pudo determinar el formato del archivo: {}", archive_path);
                return entries_array;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    let mut dict = VarDictionary::new();
                                    let name = entry.name().to_string();
                                    let size = entry.original_size() as i64;
                                    let is_dir = name.ends_with('/') || name.ends_with('\\');
                                    
                                    dict.insert("name", name);
                                    dict.insert("size", size);
                                    dict.insert("is_directory", is_dir);
                                    entries_array.push(&dict);
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error leyendo la entrada del archivo comprimido con contraseña: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {} con contraseña: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {} con contraseña: {:?}", archive_path, e);
            }
        }

        self.entries_list = entries_array.clone();
        entries_array
    }

    #[func]
    pub fn read_entry_bytes_with_password(&self, archive_path: String, entry_name: String, password: String) -> PackedByteArray {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return PackedByteArray::new();
        }
        // Primero intentamos detectar formato por la ruta habitual
        let maybe_format = ArchiveFormat::from_path(path);

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        // Si no se pudo determinar el formato directamente, revisamos si es un
        // archivo multi-volumen (ej. nombre.7z.001 o nombre.z01) y lo abrimos adecuadamente.
        if maybe_format.is_none() {
            if let Some((parts, fmt_str)) = gather_multi_volume_paths(path) {
                if fmt_str == "7z" {
                    match ArchiveFormat::open_multi_volume_7z(&parts, options) {
                        Ok(mut archive) => {
                            loop {
                                match archive.next_entry() {
                                    Ok(Some(entry)) => {
                                        if entry.name() == entry_name {
                                            let options_for_read = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                            match archive.read_with_options(&entry, &options_for_read) {
                                                Ok(data) => return PackedByteArray::from_iter(data),
                                                Err(e) => {
                                                    godot_error!("Error leyendo la entrada {} con contraseña: {:?}", entry_name, e);
                                                    return PackedByteArray::new();
                                                }
                                            }
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(e) => {
                                        godot_error!("Error iterando entradas: {:?}", e);
                                        break;
                                    }
                                }
                            }
                            godot_warn!("Entrada no encontrada en el archivo multi-volumen: {}", entry_name);
                            return PackedByteArray::new();
                        }
                        Err(e) => {
                            godot_error!("No se pudo abrir multi-volumen 7z para {}: {:?}", archive_path, e);
                            return PackedByteArray::new();
                        }
                    }
                } else if fmt_str == "zip" {
                    match ArchiveFormat::open_multi_volume_zip(&parts, options) {
                        Ok(mut archive) => {
                            loop {
                                match archive.next_entry() {
                                    Ok(Some(entry)) => {
                                        if entry.name() == entry_name {
                                            let options_for_read = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                            match archive.read_with_options(&entry, &options_for_read) {
                                                Ok(data) => return PackedByteArray::from_iter(data),
                                                Err(e) => {
                                                    godot_error!("Error leyendo la entrada {} con contraseña: {:?}", entry_name, e);
                                                    return PackedByteArray::new();
                                                }
                                            }
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(e) => {
                                        godot_error!("Error iterando entradas: {:?}", e);
                                        break;
                                    }
                                }
                            }
                            godot_warn!("Entrada no encontrada en el archivo multi-volumen: {}", entry_name);
                            return PackedByteArray::new();
                        }
                        Err(e) => {
                            godot_error!("No se pudo abrir multi-volumen zip para {}: {:?}", archive_path, e);
                            return PackedByteArray::new();
                        }
                    }
                }
            }

            godot_error!("No se pudo determinar el formato del archivo: {}", archive_path);
            return PackedByteArray::new();
        }

        let format = maybe_format.unwrap();

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    if entry.name() == entry_name {
                                        let options_for_read = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                        match archive.read_with_options(&entry, &options_for_read) {
                                            Ok(data) => {
                                                return PackedByteArray::from_iter(data);
                                            }
                                            Err(e) => {
                                                godot_error!("Error leyendo la entrada {} con contraseña: {:?}", entry_name, e);
                                                return PackedByteArray::new();
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error iterando entradas: {:?}", e);
                                    break;
                                }
                            }
                        }
                        godot_warn!("Entrada no encontrada en el archivo: {}", entry_name);
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {} con contraseña: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico: {:?}", e);
            }
        }
        PackedByteArray::new()
    }

    #[func]
    pub fn read_entry_bytes_with_format(&self, archive_path: String, entry_name: String, format_extension: String) -> PackedByteArray {
        self.read_entry_bytes_with_format_and_password(archive_path, entry_name, format_extension, String::new())
    }

    #[func]
    pub fn read_entry_bytes_with_format_and_password(&self, archive_path: String, entry_name: String, format_extension: String, password: String) -> PackedByteArray {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return PackedByteArray::new();
        }

        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("No se pudo determinar el formato forzado del archivo: {}", format_extension);
                return PackedByteArray::new();
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    if entry.name() == entry_name {
                                        let options_for_read = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                        match archive.read_with_options(&entry, &options_for_read) {
                                            Ok(data) => {
                                                return PackedByteArray::from_iter(data);
                                            }
                                            Err(e) => {
                                                godot_error!("Error leyendo la entrada {} con formato forzado y contraseña: {:?}", entry_name, e);
                                                return PackedByteArray::new();
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error iterando entradas: {:?}", e);
                                    break;
                                }
                            }
                        }
                        godot_warn!("Entrada no encontrada en el archivo: {}", entry_name);
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato forzado para {}: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico: {:?}", e);
            }
        }
        PackedByteArray::new()
    }

    #[func]
    pub fn get_entries_from_bytes(&mut self, archive_bytes: PackedByteArray, format_extension: String) -> Array<VarDictionary> {
        self.get_entries_from_bytes_with_password(archive_bytes, format_extension, String::new())
    }

    #[func]
    pub fn get_entries_from_bytes_with_password(&mut self, archive_bytes: PackedByteArray, format_extension: String, password: String) -> Array<VarDictionary> {
        let mut entries_array = Array::new();
        let bytes = archive_bytes.to_vec();
        let cursor = std::io::Cursor::new(bytes);
        
        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return entries_array;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match format.open_with_options(cursor, options) {
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
                            entries_array.push(&dict);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Error leyendo la entrada en memoria: {:?}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo desde memoria: {:?}", e);
            }
        }

        self.entries_list = entries_array.clone();
        entries_array
    }

    #[func]
    pub fn read_entry_bytes_from_bytes(&self, archive_bytes: PackedByteArray, entry_name: String, format_extension: String, password: String) -> PackedByteArray {
        let bytes = archive_bytes.to_vec();
        let cursor = std::io::Cursor::new(bytes);
        
        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return PackedByteArray::new();
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match format.open_with_options(cursor, options) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            if entry.name() == entry_name {
                                let options_for_read = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                let res = archive.read_with_options(&entry, &options_for_read);
                                
                                match res {
                                    Ok(data) => {
                                        return PackedByteArray::from_iter(data);
                                    }
                                    Err(e) => {
                                        godot_error!("Error leyendo la entrada {} en memoria: {:?}", entry_name, e);
                                        return PackedByteArray::new();
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Error iterando entradas en memoria: {:?}", e);
                            break;
                        }
                    }
                }
                godot_warn!("Entrada no encontrada en memoria: {}", entry_name);
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo en memoria: {:?}", e);
            }
        }
        PackedByteArray::new()
    }

    #[func]
    pub fn extract_all_from_bytes(&self, archive_bytes: PackedByteArray, format_extension: String, password: String) -> VarDictionary {
        let mut extracted_dict = VarDictionary::new();
        let bytes = archive_bytes.to_vec();
        let cursor = std::io::Cursor::new(bytes);
        
        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return extracted_dict;
            }
        };

        match format.open(cursor) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            let entry_name = entry.name().to_string();
                            if entry_name.ends_with('/') || entry_name.ends_with('\\') {
                                continue;
                            }
                            
                            let res = if password.is_empty() {
                                archive.read(&entry)
                            } else {
                                let options = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                archive.read_with_options(&entry, &options)
                            };

                            match res {
                                Ok(data) => {
                                    let packed = PackedByteArray::from_iter(data);
                                    let _ = extracted_dict.insert(entry_name, packed);
                                }
                                Err(e) => {
                                    godot_error!("Error extrayendo {} de memoria: {:?}", entry_name, e);
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            godot_error!("Error iterando entradas en memoria: {:?}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo en memoria: {:?}", e);
            }
        }

        extracted_dict
    }

    // LISTAR entradas de un archivo de varios volúmenes (split: .7z.001, .7z.002, .zip, .z01, etc.) sin contraseña
    #[func]
    pub fn get_entries_multi_volume(&mut self, paths: Array<GString>, format_extension: String) -> Array<VarDictionary> {
        self.get_entries_multi_volume_with_password(paths, format_extension, String::new())
    }

    // LISTAR entradas de un archivo de varios volúmenes con contraseña
    #[func]
    pub fn get_entries_multi_volume_with_password(&mut self, paths: Array<GString>, format_extension: String, password: String) -> Array<VarDictionary> {
        if paths.is_empty() {
            return Array::new();
        }

        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };
        let ext = format_extension.to_lowercase();

        let entries_array = if ext == "7z" {
            match ArchiveFormat::open_multi_volume_7z(&path_bufs, options) {
                Ok(archive) => process_archive_entries(archive),
                Err(e) => {
                    godot_error!("Error abriendo multi-volumen 7z: {:?}", e);
                    Array::new()
                }
            }
        } else if ext == "zip" {
            match ArchiveFormat::open_multi_volume_zip(&path_bufs, options) {
                Ok(archive) => process_archive_entries(archive),
                Err(e) => {
                    godot_error!("Error abriendo multi-volumen zip: {:?}", e);
                    Array::new()
                }
            }
        } else {
            match ArchiveFormat::open_path(&path_bufs[0]) {
                Ok(archive) => process_archive_entries(archive),
                Err(e) => {
                    godot_error!("Error abriendo archivo/RAR: {:?}", e);
                    Array::new()
                }
            }
        };

        self.entries_list = entries_array.clone();
        entries_array
    }

    // EXTRAER un archivo de varios volúmenes sin contraseña
    #[func]
    pub fn extract_all_multi_volume(&self, paths: Array<GString>, format_extension: String, output_dir: String) -> bool {
        self.extract_all_multi_volume_with_password(paths, format_extension, output_dir, String::new())
    }

    // EXTRAER un archivo de varios volúmenes con contraseña
    #[func]
    pub fn extract_all_multi_volume_with_password(&self, paths: Array<GString>, format_extension: String, output_dir: String, password: String) -> bool {
        if paths.is_empty() {
            return false;
        }

        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };
        let ext = format_extension.to_lowercase();
        
        let out_dir = Path::new(&output_dir);
        if let Err(e) = std::fs::create_dir_all(out_dir) {
            godot_error!("No se pudo crear el directorio de salida {}: {:?}", output_dir, e);
            return false;
        }

        if ext == "7z" {
            match ArchiveFormat::open_multi_volume_7z(&path_bufs, options) {
                Ok(archive) => extract_archive_entries(archive, out_dir),
                Err(e) => {
                    godot_error!("Error abriendo multi-volumen 7z: {:?}", e);
                    false
                }
            }
        } else if ext == "zip" {
            match ArchiveFormat::open_multi_volume_zip(&path_bufs, options) {
                Ok(archive) => extract_archive_entries(archive, out_dir),
                Err(e) => {
                    godot_error!("Error abriendo multi-volumen zip: {:?}", e);
                    false
                }
            }
        } else {
            match ArchiveFormat::open_path(&path_bufs[0]) {
                Ok(archive) => extract_archive_entries(archive, out_dir),
                Err(e) => {
                    godot_error!("Error abriendo archivo/RAR: {:?}", e);
                    false
                }
            }
        }
    }

    // Comprueba si un archivo comprimido está protegido por contraseña
    #[func]
    pub fn is_archive_encrypted(&self, archive_path: String) -> bool {
        let path = Path::new(&archive_path);
        if !path.exists() {
            return false;
        }

        match ArchiveFormat::open_path(path) {
            Ok(mut archive) => {
                loop {
                    match archive.next_entry() {
                        Ok(Some(entry)) => {
                            if entry.is_encrypted() {
                                return true;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => {
                            return true;
                        }
                    }
                }
            }
            Err(_) => {
                return true;
            }
        }
        false
    }

    // Comprueba si un archivo multi-volumen está protegido por contraseña
    #[func]
    pub fn is_archive_encrypted_multi_volume(&self, paths: Array<GString>, format_extension: String) -> bool {
        if paths.is_empty() {
            return false;
        }

        let path_bufs: Vec<std::path::PathBuf> = paths.iter_shared().map(|p| std::path::PathBuf::from(p.to_string())).collect();
        let ext = format_extension.to_lowercase();
        let options = unarc_rs::unified::ArchiveOptions::new();

        if ext == "7z" {
            match ArchiveFormat::open_multi_volume_7z(&path_bufs, options) {
                Ok(archive) => is_unified_archive_encrypted(archive),
                Err(_) => true,
            }
        } else if ext == "zip" {
            match ArchiveFormat::open_multi_volume_zip(&path_bufs, options) {
                Ok(archive) => is_unified_archive_encrypted(archive),
                Err(_) => true,
            }
        } else {
            self.is_archive_encrypted(paths.get(0).map(|p| p.to_string()).unwrap_or_default())
        }
    }

    // Comprueba si el archivo en la ruta especificada es un archivo comprimido compatible
    #[func]
    pub fn is_supported_archive(&self, archive_path: String) -> bool {
        let path = Path::new(&archive_path);
        if !path.exists() {
            return false;
        }
        is_supported_archive(path)
    }

    // Obtiene el nombre del formato comprimido a partir de la ruta del archivo
    #[func]
    pub fn get_archive_format_name(&self, archive_path: String) -> String {
        let path = Path::new(&archive_path);
        if let Some(format) = ArchiveFormat::from_path(path) {
            format.name().to_string()
        } else {
            "".to_string()
        }
    }

    // Obtiene las entradas de un archivo comprimido especificando su formato manualmente
    // (Útil para archivos con extensiones genéricas como .dat, .bin, o sin extensión)
    #[func]
    pub fn get_entries_with_format(&mut self, archive_path: String, format_extension: String) -> Array<VarDictionary> {
        let mut entries_array = Array::new();
        let path = Path::new(&archive_path);
        
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return entries_array;
        }

        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return entries_array;
            }
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open(file) {
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
                                    entries_array.push(&dict);
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error leyendo la entrada del archivo comprimido: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {}: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {}: {:?}", archive_path, e);
            }
        }

        self.entries_list = entries_array.clone();
        entries_array
    }

    // Obtiene las entradas de un archivo comprimido especificando su formato manualmente con contraseña.
    // (Útil para archivos genéricos como .dat, .bin, .miguel, o cuando el archivo está protegido)
    #[func]
    pub fn get_entries_with_format_and_password(&mut self, archive_path: String, format_extension: String, password: String) -> Array<VarDictionary> {
        let mut entries_array = Array::new();
        let path = Path::new(&archive_path);

        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return entries_array;
        }

        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return entries_array;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
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
                                    entries_array.push(&dict);
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error leyendo la entrada del archivo comprimido: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {}: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {}: {:?}", archive_path, e);
            }
        }

        self.entries_list = entries_array.clone();
        entries_array
    }

    // Extrae todo el archivo comprimido especificando su formato manualmente (sin contraseña)
    // (Útil para archivos con extensiones genéricas como .dat, .bin, o sin extensión)
    #[func]
    pub fn extract_all_with_format(&self, archive_path: String, output_dir: String, format_extension: String) -> bool {
        self.extract_all_with_format_and_password(archive_path, output_dir, format_extension, String::new())
    }

    // Extrae todo el archivo comprimido especificando su formato manualmente (soporta contraseña)
    #[func]
    pub fn extract_all_with_format_and_password(&self, archive_path: String, output_dir: String, format_extension: String, password: String) -> bool {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return false;
        }

        let out_dir = Path::new(&output_dir);
        if let Err(e) = std::fs::create_dir_all(out_dir) {
            godot_error!("No se pudo crear el directorio de salida {}: {:?}", output_dir, e);
            return false;
        }

        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return false;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    let entry_name = entry.name();
                                    let target_path = out_dir.join(entry_name);

                                    if entry_name.ends_with('/') || entry_name.ends_with('\\') {
                                        if let Err(e) = std::fs::create_dir_all(&target_path) {
                                            godot_error!("No se pudo crear el directorio {:?}: {:?}", target_path, e);
                                            return false;
                                        }
                                    } else {
                                        if let Some(parent) = target_path.parent() {
                                            if let Err(e) = std::fs::create_dir_all(parent) {
                                                godot_error!("No se pudo crear el directorio padre {:?}: {:?}", parent, e);
                                                return false;
                                            }
                                        }
                                        match std::fs::File::create(&target_path) {
                                            Ok(mut out_file) => {
                                                let res = if password.is_empty() {
                                                    archive.read_to(&entry, &mut out_file)
                                                } else {
                                                    let read_opts = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                                    archive.read_to_with_options(&entry, &mut out_file, &read_opts)
                                                };

                                                if let Err(e) = res {
                                                    godot_error!("Error escribiendo flujo al archivo {:?}: {:?}", target_path, e);
                                                    return false;
                                                }
                                            }
                                            Err(e) => {
                                                godot_error!("No se pudo crear el archivo {:?}: {:?}", target_path, e);
                                                return false;
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error iterando entradas: {:?}", e);
                                    return false;
                                }
                            }
                        }
                        true
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {}: {:?}", archive_path, e);
                        false
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {}: {:?}", archive_path, e);
                false
            }
        }
    }

    // Extrae una sola entrada del archivo a una ruta específica forzando el formato (sin contraseña)
    #[func]
    pub fn extract_entry_with_format(&self, archive_path: String, entry_name: String, dest_path: String, format_extension: String) -> bool {
        self.extract_entry_with_format_and_password(archive_path, entry_name, dest_path, format_extension, String::new())
    }

    // Extrae una sola entrada del archivo a una ruta específica forzando el formato (soporta contraseña)
    #[func]
    pub fn extract_entry_with_format_and_password(&self, archive_path: String, entry_name: String, dest_path: String, format_extension: String, password: String) -> bool {
        let path = Path::new(&archive_path);
        if !path.exists() {
            godot_warn!("El archivo no existe: {}", archive_path);
            return false;
        }

        let dummy_path = format!("dummy.{}", format_extension);
        let format = match ArchiveFormat::from_path(Path::new(&dummy_path)) {
            Some(fmt) => fmt,
            None => {
                godot_error!("Formato no soportado para la extensión: {}", format_extension);
                return false;
            }
        };

        let options = if password.is_empty() {
            unarc_rs::unified::ArchiveOptions::new()
        } else {
            unarc_rs::unified::ArchiveOptions::new().with_password(&password)
        };

        match std::fs::File::open(path) {
            Ok(file) => {
                match format.open_with_options(file, options) {
                    Ok(mut archive) => {
                        loop {
                            match archive.next_entry() {
                                Ok(Some(entry)) => {
                                    if entry.name() == entry_name {
                                        if let Some(parent) = Path::new(&dest_path).parent() {
                                            if let Err(e) = std::fs::create_dir_all(parent) {
                                                godot_error!("No se pudo crear el directorio {:?}: {:?}", parent, e);
                                                return false;
                                            }
                                        }
                                        match std::fs::File::create(&dest_path) {
                                            Ok(mut out_file) => {
                                                let res = if password.is_empty() {
                                                    archive.read_to(&entry, &mut out_file)
                                                } else {
                                                    let read_opts = unarc_rs::unified::ArchiveOptions::new().with_password(&password);
                                                    archive.read_to_with_options(&entry, &mut out_file, &read_opts)
                                                };

                                                if let Err(e) = res {
                                                    godot_error!("Error escribiendo flujo al archivo {}: {:?}", dest_path, e);
                                                    return false;
                                                }
                                                return true;
                                            }
                                            Err(e) => {
                                                godot_error!("No se pudo crear el archivo {}: {:?}", dest_path, e);
                                                return false;
                                            }
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    godot_error!("Error iterando entradas: {:?}", e);
                                    break;
                                }
                            }
                        }
                        godot_warn!("Entrada no encontrada en el archivo: {}", entry_name);
                    }
                    Err(e) => {
                        godot_error!("No se pudo abrir el lector de formato para {}: {:?}", archive_path, e);
                    }
                }
            }
            Err(e) => {
                godot_error!("No se pudo abrir el archivo físico {}: {:?}", archive_path, e);
            }
        }
        false
    }
}

fn is_unified_archive_encrypted<R: std::io::Read + std::io::Seek>(mut archive: unarc_rs::unified::UnifiedArchive<R>) -> bool {
    loop {
        match archive.next_entry() {
            Ok(Some(entry)) => {
                if entry.is_encrypted() {
                    return true;
                }
            }
            Ok(None) => break,
            Err(_) => {
                return true;
            }
        }
    }
    false
}

// FUNCIONES AUXILIARES GENÉRICAS PARA EVITAR MULTIPLICIDAD DE TIPOS UNIFIEDARCHIVE<R>
fn process_archive_entries<R: std::io::Read + std::io::Seek>(mut archive: unarc_rs::unified::UnifiedArchive<R>) -> Array<VarDictionary> {
    let mut entries_array = Array::new();
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
                entries_array.push(&dict);
            }
            Ok(None) => break,
            Err(e) => {
                godot_error!("Error leyendo la entrada del archivo multi-volumen: {:?}", e);
                break;
            }
        }
    }
    entries_array
}

fn extract_archive_entries<R: std::io::Read + std::io::Seek>(mut archive: unarc_rs::unified::UnifiedArchive<R>, out_dir: &Path) -> bool {
    loop {
        match archive.next_entry() {
            Ok(Some(entry)) => {
                let entry_name = entry.name();
                let target_path = out_dir.join(entry_name);

                if entry_name.ends_with('/') || entry_name.ends_with('\\') {
                    if let Err(e) = std::fs::create_dir_all(&target_path) {
                        godot_error!("No se pudo crear el directorio {:?}: {:?}", target_path, e);
                        return false;
                    }
                } else {
                    if let Some(parent) = target_path.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            godot_error!("No se pudo crear el directorio padre {:?}: {:?}", parent, e);
                            return false;
                        }
                    }
                    match std::fs::File::create(&target_path) {
                        Ok(mut out_file) => {
                            if let Err(e) = archive.read_to(&entry, &mut out_file) {
                                godot_error!("Error escribiendo flujo al archivo {:?}: {:?}", target_path, e);
                                return false;
                            }
                        }
                        Err(e) => {
                            godot_error!("No se pudo crear el archivo {:?}: {:?}", target_path, e);
                            return false;
                        }
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                godot_error!("Error iterando entradas multi-volumen: {:?}", e);
                return false;
            }
        }
    }
    true
}

// Detecta nombres de archivos multi-volumen y devuelve la lista de partes ordenadas
fn gather_multi_volume_paths(path: &Path) -> Option<(Vec<std::path::PathBuf>, String)> {
    let filename = path.file_name()?.to_str()?;
    let dir = path.parent().unwrap_or(Path::new("."));

    // Caso: nombre.con_ext.NUM (ej. nombre.7z.001)
    if let Some(pos) = filename.rfind('.') {
        let last = &filename[pos + 1..];
        if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
            let base = &filename[..pos]; // e.g. "kmsv3v.7z"
            let ext = Path::new(base).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

            let mut parts: Vec<std::path::PathBuf> = Vec::new();
            if let Ok(rd) = std::fs::read_dir(dir) {
                for entry in rd.flatten() {
                    if let Some(n) = entry.file_name().to_str() {
                        if n.len() > base.len() + 1 && n.starts_with(&format!("{}.", base)) {
                            let suf = &n[base.len() + 1..];
                            if !suf.is_empty() && suf.chars().all(|c| c.is_ascii_digit()) {
                                parts.push(entry.path());
                            }
                        }
                    }
                }
            }

            parts.sort_by_key(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .and_then(|n| {
                        let suf = &n[base.len() + 1..];
                        suf.parse::<u64>().ok()
                    })
                    .unwrap_or(0)
            });

            if parts.is_empty() {
                return None;
            }

            return Some((parts, ext));
        }
    }

    // Caso: zip multi-volumen con sufijos .z01, .z02 ... y archivo final .zip
    let lower = filename.to_lowercase();
    if let Some(pos) = lower.rfind('.') {
        let last = &lower[pos + 1..];
        if last.starts_with('z') && last[1..].chars().all(|c| c.is_ascii_digit()) {
            let base_prefix = &filename[..pos];
            let mut parts: Vec<std::path::PathBuf> = Vec::new();
            if let Ok(rd) = std::fs::read_dir(dir) {
                for entry in rd.flatten() {
                    if let Some(n) = entry.file_name().to_str() {
                        if n == format!("{}.zip", base_prefix) {
                            parts.push(entry.path());
                        } else if n.len() > base_prefix.len() + 1 && n.starts_with(&format!("{}.", base_prefix)) {
                            let suf = &n[base_prefix.len() + 1..];
                            if suf.starts_with('z') && suf[1..].chars().all(|c| c.is_ascii_digit()) {
                                parts.push(entry.path());
                            }
                        }
                    }
                }
            }

            parts.sort_by(|a, b| {
                let an = a.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let bn = b.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let aval = if an.ends_with(".zip") { u64::MAX } else { an.rsplit('.').next().and_then(|s| s.strip_prefix('z')).and_then(|d| d.parse::<u64>().ok()).unwrap_or(0) };
                let bval = if bn.ends_with(".zip") { u64::MAX } else { bn.rsplit('.').next().and_then(|s| s.strip_prefix('z')).and_then(|d| d.parse::<u64>().ok()).unwrap_or(0) };
                aval.cmp(&bval)
            });

            if parts.is_empty() {
                return None;
            }

            return Some((parts, "zip".to_string()));
        }
    }

    None
}
