use godot::prelude::*;
use crate::nostrn::NostrClient;
use std::sync::Mutex;

// Esta funcional 


#[derive(GodotClass)]
#[class(base=Node)]
pub struct NostrNode {
    base: Base<Node>,
    #[export]
    n_seconds: i64,
    #[export]
    n_limit: i64,
    #[export]
    poll_timeout: i64,
    client: Mutex<Option<NostrClient>>,
}



//Implementar mas ajustes

#[godot_api]
impl INode for NostrNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            n_seconds: 3600,
            n_limit: 10,
            poll_timeout: 2,
            client: Mutex::new(None),
        }
    }
}

#[godot_api]
impl NostrNode {
    /// Inicializa el cliente Nostr.
    /// Retorna true si fue exitoso, false si hubo error.
    #[func]
    pub fn init_client(&self, nsec: String, npub: String) -> bool {
        let nsec_opt = if nsec.is_empty() { None } else { Some(nsec.as_str()) };
        
        match NostrClient::new(nsec_opt, &npub) {
            Ok(client) => {
                let mut guard = self.client.lock().unwrap();
                *guard = Some(client);
                godot_print!("NostrNode: Cliente inicializado correctamente");
                true
            },
            Err(e) => {
                godot_error!("NostrNode: Error al inicializar cliente: {}", e);
                false
            }
        }
    }

    /// Agrega relays al cliente.
    /// dm_relays: Array de Strings con URLs de relays para DMs
    /// read_relays: Array de Strings con URLs de relays de lectura (opcional)
    #[func]
    pub fn add_relays(&self, dm_relays: Array<GString>, read_relays: Array<GString>) -> bool {
        let mut guard = self.client.lock().unwrap();
        if let Some(client) = guard.as_mut() {
            let dm_vec: Vec<String> = dm_relays.iter_shared().map(|s| s.to_string()).collect();
            let read_vec: Option<Vec<String>> = if read_relays.is_empty() {
                None
            } else {
                Some(read_relays.iter_shared().map(|s| s.to_string()).collect())
            };

            match client.add_relays(dm_vec, read_vec) {
                Ok(_) => {
                    godot_print!("NostrNode: Relays agregados correctamente");
                    true
                },
                Err(e) => {
                    godot_error!("NostrNode: Error al agregar relays: {}", e);
                    false
                }
            }
        } else {
            godot_error!("NostrNode: Cliente no inicializado");
            false
        }
    }

    /// Suscribe al cliente a los mensajes.
    #[func]
    pub fn subscribe(&self) -> bool {
        let mut guard = self.client.lock().unwrap();
        if let Some(client) = guard.as_mut() {
            client.n_seconds = self.n_seconds as u64;
            client.n_limit = self.n_limit as usize;
            match client.subscribe() {
                Ok(_) => {
                    godot_print!("NostrNode: Suscripción exitosa");
                    true
                },
                Err(e) => {
                    godot_error!("NostrNode: Error al suscribirse: {}", e);
                    false
                }
            }
        } else {
            godot_error!("NostrNode: Cliente no inicializado");
            false
        }
    }

    /// Envía un mensaje privado.
    #[func]
    pub fn send_message(&self, content: String) -> bool {
        let guard = self.client.lock().unwrap();
        if let Some(client) = guard.as_ref() {
            match client.send_message(&content) {
                Ok(_) => {
                    godot_print!("NostrNode: Mensaje enviado");
                    true
                },
                Err(e) => {
                    godot_error!("NostrNode: Error al enviar mensaje: {}", e);
                    false
                }
            }
        } else {
            godot_error!("NostrNode: Cliente no inicializado");
            false
        }
    }

    /// Obtiene mensajes nuevos.
    /// Retorna un Array de Dictionaries con keys: "sender", "content", "timestamp"
    #[func]
    pub fn poll_messages(&self) -> Array<VarDictionary> {
        let mut guard = self.client.lock().unwrap();
        let mut result_array = Array::new();

        if let Some(client) = guard.as_mut() {
            client.poll_timeout = self.poll_timeout as u64;
            match client.poll_messages() {
                Ok(messages) => {
                    for msg in messages {
                        let mut dict = VarDictionary::new();
                        // Convertir PublicKey a string (hex o bech32, usaremos hex para consistencia interna o bech32 si prefiere el usuario)
                        // Nota: msg.sender es XOnlyPublicKey o PublicKey dependiendo de la versión de nostr-sdk.
                        // En la versión 0.30+ es PublicKey.
                        
                        // Intentaremos usar to_bech32() si está disponible en el scope, sino hex.
                        // En client.rs se usa to_bech32() pero devuelve Result.
                        // Para simplificar en Godot, pasaremos el hex string o intentaremos bech32.
                        // Vamos a usar hex por defecto que es infalible con to_string(), 
                        // pero si queremos ser amigables con el usuario de Godot, npub es mejor.
                        // Sin embargo, to_bech32 puede fallar.
                        
                        // Vamos a usar una representación de string segura.
                        let sender_str = msg.sender.to_string(); // Hex format usually
                        
                        dict.insert("sender", sender_str);
                        dict.insert("content", msg.content);
                        dict.insert("timestamp", msg.timestamp.as_u64());
                        
                        result_array.push(&dict);
                    }
                },
                Err(e) => {
                    godot_error!("NostrNode: Error al obtener mensajes: {}", e);
                }
            }
        } else {
            godot_error!("NostrNode: Cliente no inicializado");
        }
        
        result_array
    }
    
    /// Obtiene la clave pública del usuario (npub)
    #[func]
    pub fn get_public_key(&self) -> GString {
        let guard = self.client.lock().unwrap();
        if let Some(client) = guard.as_ref() {
            match client.get_public_key() {
                Ok(pk) => (&pk).into(),
                Err(_) => "Error getting public key".into()
            }
        } else {
            "Not initialized".into()
        }
    }
}
