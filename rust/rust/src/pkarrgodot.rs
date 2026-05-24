use godot::prelude::*;
use godot::classes::{Node, FileAccess};
use godot::classes::file_access::ModeFlags;
use godot::builtin::{PackedByteArray, GString};

use std::fs::OpenOptions;
use std::io::Write;
use std::time::Instant;

use pkarr::{SignedPacket, Keypair};
use crate::state::PEER_IPS;




use pkarr::dns::Name;
use pkarr::dns::rdata::TXT;

use std::convert::TryInto;




#[derive(GodotClass)]
#[class(base=Node)]
pub struct Gpkarr {
    base: Base<Node>,
    #[export]
    pub ttl: u32,
}

#[godot_api]
impl INode for Gpkarr {
    fn init(base: Base<Node>) -> Self {
         godot_print!("Gpkarr");
        Self { 
            base,
            ttl: 30,
        }
    }
    
    fn process(&mut self, _delta: f64) {
 }
 
}



#[godot_api]
impl Gpkarr {

//probando las malditas señales
    #[signal]
    fn resolv(data: GString);
    #[signal]
    fn published(success: bool, message: GString);

//
// Helper para logging desde threads
    #[func]
    fn log_to_file(&self, msg: GString, path: GString) {
        let path_string = path.to_string();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path_string)
        {
            let _ = writeln!(file, "[{}] {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(), msg);
        }
    }


//SIN USO 
    #[func]
    fn obtener_tamano_archivo(&self, path: GString) -> u64 {
        // Intenta abrir el archivo en modo lectura (nota el 'mut' aquí)
        let mut file = match FileAccess::open(&path, ModeFlags::READ) {
            Some(f) => f,
            None => {
                godot_error!("No se pudo abrir el archivo: {}", path);
                return 0;
            }
        };

        // Obtiene el tamaño del archivo
        let length = file.get_length();
        file.close();
        length
    }

   #[func]
    pub fn key_rand(&self) -> PackedByteArray {
        let keypair = Keypair::random();
        let secret_bytes: [u8; 32] = keypair.secret_key();

        let mut packed = PackedByteArray::new();
        for byte in secret_bytes {
            packed.push(byte);
        }
        packed
    }

    #[func]
    pub fn seed_to_key(&self, seed: GString) -> PackedByteArray {
        use sha2::{Sha256, Digest};

        let seed_str = seed.to_string();
        let mut hasher = Sha256::new();
        hasher.update(seed_str.as_bytes());
        let hash = hasher.finalize();

        let mut packed = PackedByteArray::new();
        for byte in hash.iter() {
            packed.push(*byte);
        }
        packed
    }





#[func]
pub fn prepare_packet(&self, key: GString, value: GString, mode: GString, relays: PackedStringArray, keypass: PackedByteArray) -> bool {
   
   
    let bytes = keypass.to_vec();
    if bytes.len() != 32 {
        godot_error!("La clave debe tener exactamente 32 bytes, pero tiene {}", bytes.len());
        return false;
    }

    let mut secret = [0u8; 32];
    secret.copy_from_slice(&bytes);
    let keypair = Keypair::from_secret_key(&secret);
    let name_string = key.to_string();
    let value_string = value.to_string();
    let converted: Name = match name_string.as_str().try_into() {
        Ok(n) => n,
        Err(e) => {
            godot_error!("❌ Nombre inválido: {:?}", e);
            return false;
        }
    };

let txt_converted: TXT = match value_string.as_str().try_into() {
    Ok(t) => t,
    Err(e) => {
        godot_error!("❌ Valor TXT inválido: {:?}", e);
        return false;
    }
};
    
    enum Mode {
            Dht,
            Relays,
            Both,
        }

        let mode_enum = match mode.to_string().to_lowercase().as_str() {
            "dht" => Mode::Dht,
            "relays" => Mode::Relays,
            _ => Mode::Both,
        };

        let mut builder = pkarr::Client::builder();

        match mode_enum {
        Mode::Dht => {
            builder.no_relays();
        }
        Mode::Relays => {
            builder.no_dht();
            let relay_vec = relays
                .as_slice()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();

            if let Err(e) = builder.relays(&relay_vec) {
                godot_error!("❌ Error al configurar relays: {:?}", e);
                return false;
            }
        }
        Mode::Both => {
        godot_print!("modo both en #publicar , no configurado o por defecto");

        }
    }

    let client = match pkarr::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            godot_error!("Error al construir el cliente: {:?}", e);
            return false;
        }
    };

        let signed_packet = match SignedPacket::builder()
        .txt(converted, txt_converted, self.ttl)
        .sign(&keypair)
    {

        Ok(packet) => packet,
        Err(e) => {
            godot_error!("❌ Error al firmar el paquete: {:?}", e);
            return false;
        }
    };

        let instant = Instant::now();
        godot_print!("✅ Paquete firmado con clave pública: {}", keypair.public_key());
        let result = futures::executor::block_on(client.publish(&signed_packet, None));

            match result {
                Ok(()) => {
                    godot_print!(
                        "✅ Publicación exitosa: {} en {:?}",
                        keypair.public_key(),
                        instant.elapsed()
                    );
                }
                Err(err) => {
                    godot_error!(
                        "❌ Falló la publicación de {}\nError: {}",
                        keypair.public_key(),
                        err
                    );
                }
            }

                
                true
}







  
    #[func]
    pub fn resolve_key(&mut self, key: GString, mode: GString, relays: PackedStringArray)-> GString {

    enum Mode {
        Dht,
        Relays,
        Both,
    }

    let key_str = key.to_string();
    let public_key = match key_str.as_str().try_into() {
        Ok(pk) => pk,
        Err(_) => {
            godot_error!("❌ Clave zbase32 inválida");
            return "".into();
        }
    };
    // arreglar modos
    let mode_enum = match mode.to_string().to_lowercase().as_str() {
        "dht" => Mode::Dht,
        "relays" => Mode::Relays,
        _ => Mode::Both,
    };

    let mut builder = pkarr::Client::builder();

    match mode_enum {
    Mode::Dht => {
        builder.no_relays();
    }
    Mode::Relays => {
        builder.no_dht();
        let relay_vec = relays
            .as_slice()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        if let Err(e) = builder.relays(&relay_vec) {
            godot_error!("❌ Error al configurar relays: {:?}", e);
            return "".into();
        }
    }
    
    Mode::Both => {
    godot_print!("modo both o sin modo configurado correctamente en #resolver ");

    }
    
 }

    let client = match builder.build() {
        Ok(c) => c,
        Err(e) => {
            godot_error!("❌ Error al construir cliente: {:?}", e);
            return "".into();
        }
    };
    godot_print!("🔍 Resolviendo clave: {}", key_str);

    let start = std::time::Instant::now();
    let result = futures::executor::block_on(async {
        client.resolve(&public_key).await
    });

    match result {
        Some(packet) => {
            let packet_str = packet.to_string();
            godot_print!(
                "✅ Resuelto en {:?} ms: {}",
                start.elapsed().as_millis(),
                packet
            );
            return GString::from(&packet_str);
        }
        None => {
            godot_warn!("❌ Falló la resolución de {}", key_str);
            return "".into();
        }
    }
}




#[func]
pub fn public_key(&self, key: PackedByteArray) -> GString {
    let bytes = key.to_vec();

    if bytes.len() != 32 {
        godot_error!("La clave debe tener exactamente 32 bytes, pero tiene {}", bytes.len());
        return "".into();
    }

    let mut secret = [0u8; 32];
    secret.copy_from_slice(&bytes);

    let keypair = Keypair::from_secret_key(&secret);
    let pk_str = keypair.public_key().to_string();
    GString::from(&pk_str)
}


//# infoips
//test solo
  #[func]
  pub fn info_ips(&mut self) -> bool {
  {
    godot_print!("AGREGANDO IPS FICTICIAS");
    let _ruta = r"C:\Users\Emabe\Downloads\sample.torrent";
    PEER_IPS.lock().unwrap().insert("peer1".to_string(), vec!["127.0.0.1".to_string()]);
    PEER_IPS.lock().unwrap().insert("peer2".to_string(), vec![
        "127.0.0.1".to_string(),
        "128.34.56.3".to_string()
    ]);
    godot_print!("agrego una ip a peer2 ");
    if let Some(ips) = PEER_IPS.lock().unwrap().get_mut("peer2") {
        ips.push("192.168.0.1".to_string());
    }
}

{
     godot_print!("alisto los ips ");
    if let Some(ips) = PEER_IPS.lock().unwrap().get("peer2") {
            for ip in ips {
                self.base_mut().emit_signal("string_format", &[GString::from(ip).to_variant()]);
                println!("IP: {}", ip);
            }
        }
    
}

{
    godot_print!("agrego una entrada si no existe en key o si no crea la key ");
    let peer_id = "peer1".to_string();
    let ip = "127.0.0.1".to_string();

    let mut map = PEER_IPS.lock().unwrap();
    let entry = map.entry(peer_id).or_insert_with(Vec::new);

    if !entry.contains(&ip) {
        entry.push(ip);
    }
}

   
{
    godot_print!("retiro un dato de la lista");
    let mut map = PEER_IPS.lock().unwrap();
    if let Some(ips) = map.get_mut("peer1") {
        ips.retain(|x| x != "127.0.0.1");
    }
}

{
    godot_print!("solo info si existe el key ");
    let exists = PEER_IPS.lock().unwrap().contains_key("peer1".to_string().as_str());
        if exists {
            println!("ya está registrado");
            
        } else {
        godot_print!("no existe key ");
            
        }
}

    godot_print!("elimino el key ");
    PEER_IPS.lock().unwrap().remove("peer1");

    true
    }
}
