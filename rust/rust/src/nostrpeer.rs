use godot::prelude::*;
use nostr_sdk::prelude::*;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

const POW_DIFFICULTY: u8 = 2;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct NostrPeer {
    base: Base<Node>,
    #[export]
    n_seconds: i64,
    #[export]
    n_limit: i64,
    runtime: Arc<Runtime>,
    client: Arc<Mutex<Option<Client>>>,
    shared_keys: Arc<Mutex<Option<Keys>>>,
    sender_keys: Arc<Mutex<Option<Keys>>>,
    notifications: Arc<Mutex<Option<broadcast::Receiver<RelayPoolNotification>>>>,
}

#[godot_api]
impl INode for NostrPeer {
    fn init(base: Base<Node>) -> Self {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        Self {
            base,
            n_seconds: 600,
            n_limit: 10,
            runtime: Arc::new(runtime),
            client: Arc::new(Mutex::new(None)),
            shared_keys: Arc::new(Mutex::new(None)),
            sender_keys: Arc::new(Mutex::new(None)),
            notifications: Arc::new(Mutex::new(None)),
        }
    }
}

#[godot_api]
impl NostrPeer {
    #[func]
    pub fn init_participant(&self, sender_secret: String, receiver_pubkey: String, relay_urls: Array<GString>) -> bool {
        let runtime = self.runtime.clone();
        let client_store = self.client.clone();
        let shared_keys_store = self.shared_keys.clone();
        let sender_keys_store = self.sender_keys.clone();
        let notifications_store = self.notifications.clone();
        let limit = self.n_limit as usize;

        let res = runtime.block_on(async move {
            let sk = Keys::parse(&sender_secret).map_err(|e| format!("Invalid sender secret: {}", e))?;
            let receiver = PublicKey::from_str(&receiver_pubkey).map_err(|e| format!("Invalid receiver pubkey: {}", e))?;

            let shared_key_bytes = nostr_sdk::util::generate_shared_key(sk.secret_key(), &receiver)
                .map_err(|e| format!("Error generating shared key: {}", e))?;
            let shared_secret_key = SecretKey::from_slice(&shared_key_bytes).map_err(|e| format!("Invalid shared key bytes: {}", e))?;
            let shared_keys = Keys::new(shared_secret_key);

            godot_print!("Shared Key: {}", shared_keys.secret_key().to_secret_hex());
            godot_print!("Mode: Participant");

            let client = Client::new(Keys::generate());
            
            // Add all relays
            for relay_url in relay_urls.iter_shared() {
                let url_str = relay_url.to_string();
                godot_print!("Adding relay: {}", url_str);
                client.add_relay(&url_str).await.map_err(|e| format!("Failed to add relay {}: {}", url_str, e))?;
            }
            
            client.connect().await;

            let filter = Filter::new()
                .kind(Kind::GiftWrap)
                .limit(limit)
                .pubkey(shared_keys.public_key());
            
            client.subscribe(filter, None).await.map_err(|e| format!("Failed to subscribe: {}", e))?;

            *client_store.lock().unwrap() = Some(client.clone());
            *shared_keys_store.lock().unwrap() = Some(shared_keys);
            *sender_keys_store.lock().unwrap() = Some(sk);
            *notifications_store.lock().unwrap() = Some(client.notifications());

            Ok::<(), String>(())
        });

        match res {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[func]
    pub fn init_observer(&self, shared_key_hex: String, relay_urls: Array<GString>) -> bool {
        let runtime = self.runtime.clone();
        let client_store = self.client.clone();
        let shared_keys_store = self.shared_keys.clone();
        let sender_keys_store = self.sender_keys.clone();
        let notifications_store = self.notifications.clone();
        let limit = self.n_limit as usize;

        let res = runtime.block_on(async move {
            let shared_secret_key = SecretKey::from_str(&shared_key_hex).map_err(|e| format!("Invalid shared key: {}", e))?;
            let shared_keys = Keys::new(shared_secret_key);

            godot_print!("Shared Key: {}", shared_keys.secret_key().to_secret_hex());
            godot_print!("Mode: Observer");

            let client = Client::new(Keys::generate());
            
            // Add all relays
            for relay_url in relay_urls.iter_shared() {
                let url_str = relay_url.to_string();
                godot_print!("Adding relay: {}", url_str);
                client.add_relay(&url_str).await.map_err(|e| format!("Failed to add relay {}: {}", url_str, e))?;
            }
            
            client.connect().await;

            let filter = Filter::new()
                .kind(Kind::GiftWrap)
                .limit(limit)
                .pubkey(shared_keys.public_key());
            
            client.subscribe(filter, None).await.map_err(|e| format!("Failed to subscribe: {}", e))?;

            *client_store.lock().unwrap() = Some(client.clone());
            *shared_keys_store.lock().unwrap() = Some(shared_keys);
            *sender_keys_store.lock().unwrap() = None;
            *notifications_store.lock().unwrap() = Some(client.notifications());

            Ok::<(), String>(())
        });

        match res {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[func]
    pub fn send_message(&self, message: String) -> bool {
        let runtime = self.runtime.clone();
        let client_guard = self.client.lock().unwrap();
        let sender_guard = self.sender_keys.lock().unwrap();
        let shared_guard = self.shared_keys.lock().unwrap();

        if let (Some(client), Some(sender), Some(shared)) = (client_guard.as_ref(), sender_guard.as_ref(), shared_guard.as_ref()) {
            let client = client.clone();
            let sender = sender.clone();
            let shared_pub = shared.public_key();
            let message = message.clone();

            let res = runtime.block_on(async move {
                let wrapped_event = mostro_wrap(&sender, shared_pub, &message, vec![]).await
                    .map_err(|e| format!("Error wrapping message: {}", e))?;
                client.send_event(&wrapped_event).await.map_err(|e| format!("Error sending event: {}", e))?;
                Ok::<(), String>(())
            });

            match res {
                Ok(_) => true,
                Err(e) => {
                    godot_error!("{}", e);
                    false
                }
            }
        } else {
            godot_error!("Client, Sender or Shared keys not initialized");
            false
        }
    }

    #[func]
    pub fn poll_messages(&self) -> Array<VarDictionary> {
        let runtime = self.runtime.clone();
        let mut notifications_guard = self.notifications.lock().unwrap();
        let shared_guard = self.shared_keys.lock().unwrap();
        
        let mut messages = Array::new();

        if let (Some(notifications), Some(shared_keys)) = (notifications_guard.as_mut(), shared_guard.as_ref()) {
            let shared_keys = shared_keys.clone();
            
            loop {
                match notifications.try_recv() {
                    Ok(notification) => {
                         if let RelayPoolNotification::Event { event, .. } = notification {
                             let res = runtime.block_on(async {
                                 if let Ok(inner_event) = mostro_unwrap(&shared_keys, *event).await {
                                     let now = Timestamp::now().as_u64();
                                     let msg_time = inner_event.created_at.as_u64();
                                     if now.saturating_sub(msg_time) > self.n_seconds as u64 {
                                         return None;
                                     }
                                     Some(inner_event)
                                 } else {
                                     None
                                 }
                             });

                             if let Some(inner_event) = res {
                                 let mut dict = VarDictionary::new();
                                 dict.set("pubkey", inner_event.pubkey.to_string());
                                 dict.set("content", inner_event.content);
                                 dict.set("created_at", inner_event.created_at.as_u64());
                                 messages.push(&dict);
                             }
                         }
                    },
                    Err(broadcast::error::TryRecvError::Empty) => break,
                    Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                    Err(broadcast::error::TryRecvError::Closed) => break,
                }
            }
        }

        messages
    }
}

async fn mostro_wrap(
    sender: &Keys,
    receiver: PublicKey,
    message: &str,
    extra_tags: Vec<Tag>,
) -> Result<Event, Box<dyn std::error::Error>> {
    let inner_event = EventBuilder::text_note(message)
        .build(sender.public_key())
        .sign(sender)
        .await?;
    
    let keys: Keys = Keys::generate();
    let encrypted_content: String = nip44::encrypt(
        keys.secret_key(),
        &receiver,
        inner_event.as_json(),
        nip44::Version::V2,
    )?;

    let mut tags = vec![Tag::public_key(receiver)];
    tags.extend(extra_tags);

    let wrapped_event = EventBuilder::new(Kind::GiftWrap, encrypted_content)
        .pow(POW_DIFFICULTY)
        .tags(tags)
        .custom_created_at(Timestamp::tweaked(nip59::RANGE_RANDOM_TIMESTAMP_TWEAK))
        .sign_with_keys(&keys)?;
    Ok(wrapped_event)
}

async fn mostro_unwrap(
    receiver: &Keys,
    event: Event,
) -> Result<Event, Box<dyn std::error::Error>> {
    let decrypted_content = nip44::decrypt(receiver.secret_key(), &event.pubkey, &event.content)?;
    let inner_event = Event::from_json(&decrypted_content)?;
    inner_event.verify()?;
    Ok(inner_event)
}