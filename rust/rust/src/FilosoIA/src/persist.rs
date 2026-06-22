use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use argon2::Argon2;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;

const CONFIG_FILE: &str = "config.json";
const COOLDOWN_SECS: u64 = 300;
const CALL_WINDOW_SECS: u64 = 3600;
const SALT: &[u8] = b"opencode-chat-salt-v1";

#[derive(Serialize, Deserialize, Clone)]
pub struct KeyEntry {
    pub key: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub keys: Vec<KeyEntry>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default = "default_healthy")]
    pub healthy: bool,
    pub cooldown_until: Option<u64>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub blocked_models: Vec<String>,
    #[serde(default)]
    pub custom_models: Vec<String>,
}

fn default_healthy() -> bool { true }

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    providers: HashMap<String, ProviderConfig>,
}

#[derive(Serialize, Deserialize)]
struct EncryptedFile {
    encrypted: bool,
    data: String,
}

#[derive(Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Clone)]
pub struct ProviderState {
    pub calls: u32,
    pub window_start: Instant,
}

pub struct Persist {
    config_path: PathBuf,
    pub providers: HashMap<String, ProviderConfig>,
    pub states: HashMap<String, ProviderState>,
    pub token_usage: HashMap<String, TokenUsage>,
    pub password: Option<String>,
}

fn derive_key(password: &str) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default().hash_password_into(password.as_bytes(), SALT, &mut key)
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(key)
}

fn encrypt(data: &[u8], password: &str) -> Result<String, String> {
    let key_bytes = derive_key(password)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| format!("encryption failed: {e}"))?;
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

fn decrypt(data: &str, password: &str) -> Result<Vec<u8>, String> {
    let combined = BASE64.decode(data)
        .map_err(|e| format!("base64 decode failed: {e}"))?;
    if combined.len() < 12 {
        return Err("invalid encrypted data".into());
    }
    let key_bytes = derive_key(password)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&combined[..12]);
    let plaintext = cipher.decrypt(nonce, &combined[12..])
        .map_err(|e| format!("decryption failed: {e}"))?;
    Ok(plaintext)
}

impl Persist {
    pub fn load(password: Option<String>) -> Self {
        let path = PathBuf::from(CONFIG_FILE);
        let providers = if path.exists() {
            let raw = std::fs::read_to_string(&path).unwrap_or_default();
            let try_encrypted: Result<EncryptedFile, _> = serde_json::from_str(&raw);
            if let Ok(ef) = try_encrypted {
                if ef.encrypted {
                    if let Some(ref pw) = password {
                        match decrypt(&ef.data, pw) {
                            Ok(plain) => {
                                serde_json::from_slice::<ConfigFile>(&plain)
                                    .ok()
                                    .map(|c| c.providers)
                                    .unwrap_or_default()
                            }
                            Err(_) => {
                                eprintln!("Wrong password or corrupted config.");
                                HashMap::new()
                            }
                        }
                    } else {
                        eprintln!("Config is encrypted but no password given.");
                        HashMap::new()
                    }
                } else {
                    HashMap::new()
                }
            } else {
                serde_json::from_str::<ConfigFile>(&raw)
                    .ok()
                    .map(|c| c.providers)
                    .unwrap_or_default()
            }
        } else {
            HashMap::new()
        };
        Persist {
            config_path: path,
            states: HashMap::new(),
            token_usage: HashMap::new(),
            providers,
            password,
        }
    }

    pub fn save(&self) {
        let file = ConfigFile {
            providers: self.providers.clone(),
        };
        if let Some(ref pw) = self.password {
            if let Ok(json) = serde_json::to_vec(&file) {
                if let Ok(enc) = encrypt(&json, pw) {
                    let ef = EncryptedFile { encrypted: true, data: enc };
                    if let Ok(out) = serde_json::to_string_pretty(&ef) {
                        let _ = std::fs::write(&self.config_path, out);
                    }
                }
            }
        } else {
            if let Ok(json) = serde_json::to_string_pretty(&file) {
                let _ = std::fs::write(&self.config_path, json);
            }
        }
    }

    pub fn get_key(&self, provider: &str) -> Option<String> {
        self.providers.get(provider).and_then(|cfg| {
            cfg.keys.iter()
                .find(|k| k.is_default)
                .or_else(|| cfg.keys.first())
                .map(|k| k.key.clone())
        })
    }

    pub fn get_keys(&self, provider: &str) -> Vec<(usize, &KeyEntry)> {
        self.providers.get(provider)
            .map(|cfg| cfg.keys.iter().enumerate().collect())
            .unwrap_or_default()
    }

    pub fn set_key(&mut self, provider: &str, key: String) {
        let entry = self.providers.entry(provider.to_string());
        let cfg = entry.or_insert_with(|| ProviderConfig {
            keys: vec![],
            metadata: HashMap::new(),
            healthy: true,
            cooldown_until: None,
            last_error: None,
            blocked_models: vec![],
            custom_models: vec![],
        });
        if let Some(existing) = cfg.keys.iter_mut().find(|k| k.is_default) {
            existing.key = key;
        } else {
            cfg.keys.push(KeyEntry { key, is_default: true });
        }
        self.save();
    }

    pub fn add_key(&mut self, provider: &str, key: String) {
        let entry = self.providers.entry(provider.to_string());
        let cfg = entry.or_insert_with(|| ProviderConfig {
            keys: vec![],
            metadata: HashMap::new(),
            healthy: true,
            cooldown_until: None,
            last_error: None,
            blocked_models: vec![],
            custom_models: vec![],
        });
        cfg.keys.push(KeyEntry { key, is_default: cfg.keys.is_empty() });
        self.save();
    }

    pub fn remove_key(&mut self, provider: &str, index: usize) -> bool {
        if let Some(cfg) = self.providers.get_mut(provider) {
            if index < cfg.keys.len() {
                let removed = cfg.keys.remove(index);
                if removed.is_default && !cfg.keys.is_empty() {
                    cfg.keys[0].is_default = true;
                }
                self.save();
                return true;
            }
        }
        false
    }

    pub fn set_default_key(&mut self, provider: &str, index: usize) -> bool {
        if let Some(cfg) = self.providers.get_mut(provider) {
            if index < cfg.keys.len() {
                for k in cfg.keys.iter_mut() {
                    k.is_default = false;
                }
                cfg.keys[index].is_default = true;
                self.save();
                return true;
            }
        }
        false
    }

    pub fn rotate_key(&mut self, provider: &str) {
        if let Some(cfg) = self.providers.get_mut(provider) {
            if cfg.keys.len() > 1 {
                let idx = cfg.keys.iter().position(|k| k.is_default).unwrap_or(0);
                cfg.keys[idx].is_default = false;
                let next = (idx + 1) % cfg.keys.len();
                cfg.keys[next].is_default = true;
                self.save();
            }
        }
    }

    pub fn get_metadata(&self, provider: &str, key: &str) -> Option<String> {
        self.providers.get(provider)
            .and_then(|cfg| cfg.metadata.get(key).cloned())
    }

    pub fn set_metadata(&mut self, provider: &str, key: String, value: String) {
        let entry = self.providers.entry(provider.to_string());
        let cfg = entry.or_insert_with(|| ProviderConfig {
            keys: vec![],
            metadata: HashMap::new(),
            healthy: true,
            cooldown_until: None,
            last_error: None,
            blocked_models: vec![],
            custom_models: vec![],
        });
        cfg.metadata.insert(key, value);
        self.save();
    }

    pub fn mark_error(&mut self, provider: &str, error: &str) {
        let until = (std::time::SystemTime::now()
            + Duration::from_secs(COOLDOWN_SECS))
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        if let Some(cfg) = self.providers.get_mut(provider) {
            cfg.healthy = false;
            cfg.cooldown_until = until;
            cfg.last_error = Some(error.to_string());
        }
        self.save();
    }

    pub fn mark_healthy(&mut self, provider: &str) {
        if let Some(cfg) = self.providers.get_mut(provider) {
            cfg.healthy = true;
            cfg.cooldown_until = None;
            cfg.last_error = None;
        }
        self.save();
    }

    pub fn is_on_cooldown(&self, provider: &str) -> bool {
        self.providers.get(provider).and_then(|cfg| {
            cfg.cooldown_until.map(|until| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                now < until
            })
        }).unwrap_or(false)
    }

    pub fn track_call(&mut self, provider: &str) {
        let state = self.states.entry(provider.to_string()).or_insert(ProviderState {
            calls: 0,
            window_start: Instant::now(),
        });
        if state.window_start.elapsed() > Duration::from_secs(CALL_WINDOW_SECS) {
            state.calls = 0;
            state.window_start = Instant::now();
        }
        state.calls += 1;
    }

    pub fn calls_in_window(&self, provider: &str) -> u32 {
        self.states.get(provider)
            .filter(|s| s.window_start.elapsed() <= Duration::from_secs(CALL_WINDOW_SECS))
            .map(|s| s.calls)
            .unwrap_or(0)
    }

    pub fn track_tokens(&mut self, provider: &str, prompt: u64, completion: u64) {
        let entry = self.token_usage.entry(provider.to_string()).or_default();
        entry.prompt_tokens += prompt;
        entry.completion_tokens += completion;
    }

    pub fn get_token_usage(&self, provider: &str) -> (u64, u64) {
        self.token_usage.get(provider)
            .map(|t| (t.prompt_tokens, t.completion_tokens))
            .unwrap_or((0, 0))
    }

    pub fn block_model(&mut self, provider: &str, model: &str) {
        let cfg = self.providers.entry(provider.to_string()).or_insert_with(|| ProviderConfig {
            keys: vec![], metadata: HashMap::new(), healthy: true,
            cooldown_until: None, last_error: None, blocked_models: vec![], custom_models: vec![],
        });
        if !cfg.blocked_models.contains(&model.to_string()) {
            cfg.blocked_models.push(model.to_string());
            self.save();
        }
    }

    pub fn unblock_model(&mut self, provider: &str, model: &str) {
        if let Some(cfg) = self.providers.get_mut(provider) {
            cfg.blocked_models.retain(|m| m != model);
            self.save();
        }
    }

    pub fn get_blocked_models(&self, provider: &str) -> Vec<String> {
        self.providers.get(provider)
            .map(|cfg| cfg.blocked_models.clone())
            .unwrap_or_default()
    }

    pub fn is_model_blocked(&self, provider: &str, model: &str) -> bool {
        self.providers.get(provider)
            .map(|cfg| cfg.blocked_models.contains(&model.to_string()))
            .unwrap_or(false)
    }

    pub fn add_custom_model(&mut self, provider: &str, model: &str) {
        let cfg = self.providers.entry(provider.to_string()).or_insert_with(|| ProviderConfig {
            keys: vec![], metadata: HashMap::new(), healthy: true,
            cooldown_until: None, last_error: None, blocked_models: vec![], custom_models: vec![],
        });
        if !cfg.custom_models.contains(&model.to_string()) {
            cfg.custom_models.push(model.to_string());
            self.save();
        }
    }

    pub fn remove_custom_model(&mut self, provider: &str, index: usize) -> bool {
        if let Some(cfg) = self.providers.get_mut(provider) {
            if index < cfg.custom_models.len() {
                cfg.custom_models.remove(index);
                self.save();
                return true;
            }
        }
        false
    }

    pub fn get_custom_models(&self, provider: &str) -> Vec<String> {
        self.providers.get(provider)
            .map(|cfg| cfg.custom_models.clone())
            .unwrap_or_default()
    }
}
