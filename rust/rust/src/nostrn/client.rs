use anyhow::{anyhow, Context, Result};
use nostr::prelude::*;
use nostr_sdk::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, Mutex};

use super::relays::{compose_write_relays, connect_relays};

/// Estructura que almacena la configuración del cliente Nostr para reutilización.
/// No usa threads ni channels - todas las operaciones son síncronas desde la perspectiva del llamador.
pub struct NostrClient {
    client: Client,
    keys: Keys,
    peer_pk: PublicKey,
    runtime: Arc<Runtime>,
    subscription_id: Option<SubscriptionId>,
    notifications: Arc<Mutex<Option<broadcast::Receiver<RelayPoolNotification>>>>,
    pub n_seconds: u64,
    pub n_limit: usize,
    pub poll_timeout: u64,
}

impl NostrClient {
    /// Crea un nuevo cliente Nostr con las claves y el destinatario especificados.
    /// 
    /// # Argumentos
    /// * `nsec` - Clave secreta en formato nsec (opcional, se generará una si es None)
    /// * `recipient_npub` - Clave pública del destinatario en formato npub
    pub fn new(nsec: Option<&str>, recipient_npub: &str) -> Result<Self> {
        // Parsear o generar claves
        let keys = if let Some(s) = nsec {
            Keys::parse(&s).context("Failed to parse provided secret key")?
        } else {
            Keys::generate()
        };

        // Parsear destinatario
        let peer_pk = PublicKey::from_bech32(recipient_npub)
            .context("Failed to parse recipient npub")?;

        // Crear cliente
        let client = Client::builder()
            .signer(keys.clone())
            .build();

        // Crear runtime de Tokio
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("Failed to create Tokio runtime")?
        );

        Ok(Self {
            client,
            keys,
            peer_pk,
            runtime,
            subscription_id: None,
            notifications: Arc::new(Mutex::new(None)),
            n_seconds: 3600,
            n_limit: 10,
            poll_timeout: 2,
        })
    }

    /// Agrega y conecta a los relays especificados.
    /// 
    /// # Argumentos
    /// * `dm_relays` - Lista de relays DM del destinatario
    /// * `read_relays` - Lista de relays de lectura (opcional, usa dm_relays si está vacío)
    pub fn add_relays(&mut self, dm_relays: Vec<String>, read_relays: Option<Vec<String>>) -> Result<()> {
        if dm_relays.is_empty() {
            return Err(anyhow!(
                "Strict NIP-17 mode requires at least one DM relay"
            ));
        }

        // Determinar relays de lectura
        let read = if let Some(r) = read_relays {
            if r.is_empty() {
                dm_relays.clone()
            } else {
                r
            }
        } else {
            dm_relays.clone()
        };

        // Componer relays de escritura (DM + fallbacks)
        let write = compose_write_relays(&dm_relays);

        // Conectar usando el runtime
        self.runtime.block_on(async {
            connect_relays(&self.client, &read, &write).await
        })?;

        Ok(())
    }

    /// Inicia la suscripción para recibir mensajes.
    /// Debe llamarse después de add_relays() y antes de poll_messages().
    pub fn subscribe(&mut self) -> Result<()> {
        // Fetch messages based on n_seconds
        // Filter 1: Messages sent TO me (any kind)
        let filter_to_me = Filter::new()
            .pubkey(self.keys.public_key())
            .limit(self.n_limit)
            .since(Timestamp::from(Timestamp::now().as_u64().saturating_sub(self.n_seconds)));
            
        // Filter 2: Messages FROM the peer (any kind)
        let filter_from_peer = Filter::new()
            .author(self.peer_pk)
            .limit(self.n_limit)
            .since(Timestamp::from(Timestamp::now().as_u64().saturating_sub(self.n_seconds)));

        println!("      [DEBUG] Suscribiendo a:");
        println!("      - Mensajes para mi: {}", self.keys.public_key().to_bech32()?);
        println!("      - Mensajes del peer: {}", self.peer_pk.to_bech32()?);

        let sub_id = self.runtime.block_on(async {
            // Subscribe to messages TO me
            let id1 = self.client.subscribe(filter_to_me, None).await?;
            println!("      [DEBUG] Suscripción 1 ID: {}", id1.id());
            
            // Subscribe to messages FROM peer
            let id2 = self.client.subscribe(filter_from_peer, None).await?;
            println!("      [DEBUG] Suscripción 2 ID: {}", id2.id());
            
            Ok::<_, anyhow::Error>(id1)
        })?;

        self.subscription_id = Some(sub_id.id().clone());
        
        // Inicializar el receiver de notificaciones UNA VEZ
        let notifications_receiver = self.client.notifications();
        self.runtime.block_on(async {
            *self.notifications.lock().await = Some(notifications_receiver);
        });
        
        Ok(())
    }
    /// Envía un mensaje privado al destinatario.
    /// 
    /// # Argumentos
    /// * `message` - Contenido del mensaje a enviar
    pub fn send_message(&self, message: &str) -> Result<()> {
        self.runtime.block_on(async {
            self.client
                .send_private_msg(self.peer_pk, message, [])
                .await
                .context("Failed to send private message")
        })?;
        Ok(())
    }

    /// Obtiene los mensajes recibidos desde el peer especificado.
    /// Espera hasta 2 segundos para que lleguen mensajes de los relays.
    /// 
    /// # Retorna
    /// Vector de mensajes recibidos (puede estar vacío si no hay mensajes nuevos)
    pub fn poll_messages(&self) -> Result<Vec<ReceivedMessage>> {
        let mut messages = Vec::new();
        let mut event_count = 0;
        let mut system_count = 0;
        let mut giftwrap_count = 0;
        
        self.runtime.block_on(async {
            // Reutilizar el receiver de notificaciones existente
            let mut notifications_guard = self.notifications.lock().await;
            if notifications_guard.is_none() {
                println!("      [DEBUG] ⚠️  No hay receiver de notificaciones. ¿Llamaste a subscribe()?");
                return;
            }
            
            let notifications = notifications_guard.as_mut().unwrap();
            let timeout_duration = tokio::time::Duration::from_secs(self.poll_timeout);
            let deadline = tokio::time::Instant::now() + timeout_duration;
            
            println!("      [DEBUG] Esperando notificaciones por {} segundos...", self.poll_timeout);
            
            // Esperar mensajes hasta el timeout
            loop {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                
                match tokio::time::timeout(remaining, notifications.recv()).await {
                    Ok(Ok(notification)) => {
                        // Loguear qué tipo de notificación es
                        match &notification {
                            RelayPoolNotification::Event { relay_url, event, .. } => {
                                event_count += 1;
                                println!("      [DEBUG] Evento recibido de {}: Kind={}, ID={}", 
                                         relay_url, event.kind, event.id);
                                if event.kind == Kind::GiftWrap {
                                    giftwrap_count += 1;
                                    println!("      [DEBUG] GiftWrap recibido, intentando desencriptar...");
                                    match self.client.unwrap_gift_wrap(&event).await {
                                        Ok(UnwrappedGift { sender, rumor }) => {
                                            println!("      [DEBUG] Desencriptado exitoso. Sender: {}, Kind: {}", 
                                                     sender.to_hex().chars().take(8).collect::<String>(),
                                                     rumor.kind);
                                            // Solo mostrar mensajes del peer seleccionado
                                            if rumor.kind == Kind::PrivateDirectMessage && sender == self.peer_pk {
                                                println!("      [DEBUG] ✓ Mensaje del peer correcto!");
                                                messages.push(ReceivedMessage {
                                                    sender,
                                                    content: rumor.content,
                                                    timestamp: rumor.created_at,
                                                });
                                            } else {
                                                println!("      [DEBUG] ✗ Mensaje de otro peer o tipo incorrecto");
                                            }
                                        }
                                        Err(e) => {
                                            println!("      [DEBUG] Error desencriptando: {}", e);
                                        }
                                    }
                                } else if event.kind == Kind::EncryptedDirectMessage {
                                    println!("      [DEBUG] ⚠️ Recibido mensaje legado (Kind 4). Este cliente espera NIP-17 (GiftWrap).");
                                }
                            },
                            RelayPoolNotification::Message { relay_url, message } => {
                                system_count += 1;
                             //   println!("      [DEBUG] Mensaje de sistema de {}: {:?}", relay_url, message);
                            },
                            RelayPoolNotification::Shutdown => {
                                println!("      [DEBUG] Notificación de Shutdown");
                            },

                        }
                    }
                    Ok(Err(_)) | Err(_) => break, // Error o timeout
                }
            }
            
            println!("      [DEBUG] Resumen: {} Eventos, {} Mensajes de Sistema (EOSE/OK), {} GiftWraps, {} Mensajes Válidos", 
                     event_count, system_count, giftwrap_count, messages.len());
        });

        Ok(messages)
    }

    /// Obtiene la clave pública del usuario.
    pub fn get_public_key(&self) -> Result<String> {
        Ok(self.keys.public_key().to_bech32()?)
    }

    /// Obtiene la clave pública del destinatario.
    pub fn get_peer_public_key(&self) -> Result<String> {
        Ok(self.peer_pk.to_bech32()?)
    }

    /// Desconecta del cliente y limpia recursos.
    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(sub_id) = &self.subscription_id {
            self.runtime.block_on(async {
                self.client.unsubscribe(&sub_id).await;
            });
            self.subscription_id = None;
        }
        
        self.runtime.block_on(async {
            self.client.disconnect().await
        });
        
        Ok(())
    }
}

/// Representa un mensaje recibido
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub sender: PublicKey,
    pub content: String,
    pub timestamp: Timestamp,
}

impl Drop for NostrClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}
