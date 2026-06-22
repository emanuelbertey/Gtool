use godot::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::runtime::Runtime;
use openrouter_chat::tools;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::Argon2;
use base64::Engine;

const SALT: &[u8] = b"opencode-chat-salt-v1";

fn derive_key(password: &str) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), SALT, &mut key)
        .map_err(|e| format!("Argon2 failed: {:?}", e))?;
    Ok(key)
}

fn decrypt_json(data: &str, password: &str) -> Result<String, String> {
    let engine = base64::engine::general_purpose::STANDARD;
    let combined = engine.decode(data).map_err(|e| format!("Base64: {}", e))?;
    if combined.len() < 12 {
        return Err("Data too short".into());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let key = derive_key(password)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("AES: {:?}", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decrypt failed: {:?}", e))?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8: {}", e))
}

fn encrypt_json(plaintext: &str, password: &str) -> Result<String, String> {
    let key = derive_key(password)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("AES: {:?}", e))?;
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encrypt failed: {:?}", e))?;
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    let engine = base64::engine::general_purpose::STANDARD;
    Ok(engine.encode(&combined))
}

fn provider_base_url(name: &str) -> Option<&'static str> {
    match name {
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "xai" => Some("https://api.x.ai/v1"),
        "groq" => Some("https://api.groq.com/openai/v1"),
        "nvidia" => Some("https://integrate.api.nvidia.com/v1"),
        "cerebras" => Some("https://api.cerebras.ai/v1"),
        "mistral" => Some("https://api.mistral.ai/v1"),
        "cloudflare" => Some("https://api.cloudflare.com/client/v4/accounts"),
        "github" => Some("https://models.inference.ai.azure.com"),
        "gemini" => Some("https://generativelanguage.googleapis.com/v1beta/openai/"),
        "llm7" => Some("https://api.llm7.io/v1"),
        "kilo" => Some("https://api.kilo.ai/api/gateway"),
        _ => None,
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Gchat {
    config: Option<HashMap<String, serde_json::Value>>,
    password: String,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Gchat {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            config: None,
            password: String::new(),
            base,
        }
    }
}

#[godot_api]
impl Gchat {
    #[func]
    pub fn load_config(&mut self, path: GString, password: GString) -> Dictionary {
        let path_str = path.to_string();
        let pw = password.to_string();
        let content = match std::fs::read_to_string(&path_str) {
            Ok(c) => c,
            Err(e) => {
                godot_error!("Gchat: Failed to read {}: {}", path_str, e);
                return Dictionary::new();
            }
        };
        let parsed: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("Gchat: Invalid JSON: {}", e);
                return Dictionary::new();
            }
        };
        let is_encrypted = parsed.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false);
        let decrypted = if is_encrypted {
            let data_str = parsed.get("data").and_then(|v| v.as_str()).unwrap_or("");
            match decrypt_json(data_str, &pw) {
                Ok(p) => p,
                Err(e) => {
                    godot_error!("Gchat: Decrypt failed: {}", e);
                    return Dictionary::new();
                }
            }
        } else {
            content
        };
        let config_val: serde_json::Value = match serde_json::from_str(&decrypted) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("Gchat: Invalid decrypted JSON: {}", e);
                return Dictionary::new();
            }
        };
        self.password = pw;
        let mut config_map = HashMap::new();
        if let Some(providers) = config_val.get("providers").and_then(|v| v.as_object()) {
            for (name, data) in providers {
                config_map.insert(name.clone(), data.clone());
            }
        }
        self.config = Some(config_map);

        let mut dict = Dictionary::new();
        if let Some(ref cfg) = self.config {
            for (name, data) in cfg {
                let val_str = serde_json::to_string(data).unwrap_or_default();
                dict.set(name.as_str(), GString::from(&val_str));
            }
        }
        godot_print!("Gchat: Loaded {} providers from {}", dict.len(), path_str);
        dict
    }

    #[func]
    pub fn save_config(&self, path: GString, password: GString) -> bool {
        let cfg = match &self.config {
            Some(c) => c,
            None => {
                godot_error!("Gchat: No config loaded");
                return false;
            }
        };
        let mut providers_map = serde_json::Map::new();
        for (name, data) in cfg {
            providers_map.insert(name.clone(), data.clone());
        }
        let root = serde_json::json!({ "providers": providers_map });
        let plaintext = serde_json::to_string_pretty(&root).unwrap_or_default();
        let data_b64 = match encrypt_json(&plaintext, &password.to_string()) {
            Ok(d) => d,
            Err(e) => {
                godot_error!("Gchat: Encrypt failed: {}", e);
                return false;
            }
        };
        let output = serde_json::json!({ "encrypted": true, "data": data_b64 });
        let out_str = serde_json::to_string_pretty(&output).unwrap_or_default();
        match std::fs::write(path.to_string(), out_str) {
            Ok(_) => {
                godot_print!("Gchat: Config saved");
                true
            }
            Err(e) => {
                godot_error!("Gchat: Write failed: {}", e);
                false
            }
        }
    }

    #[func]
    pub fn list_providers(&self) -> Array<GString> {
        let mut arr = Array::new();
        if let Some(ref cfg) = self.config {
            for name in cfg.keys() {
                arr.push(&GString::from(name));
            }
        }
        arr
    }

    #[func]
    pub fn get_keys(&self, provider: GString) -> Array<GString> {
        let mut arr = Array::new();
        let name = provider.to_string();
        if let Some(ref cfg) = self.config {
            if let Some(data) = cfg.get(&name) {
                if let Some(keys) = data.get("keys").and_then(|v| v.as_array()) {
                    for entry in keys {
                        if let Some(k) = entry.get("key").and_then(|v| v.as_str()) {
                            arr.push(&GString::from(k));
                        }
                    }
                }
            }
        }
        arr
    }

    #[func]
    pub fn set_keys(&mut self, provider: GString, keys: Array<GString>) -> bool {
        let name = provider.to_string();
        let cfg = self.config.get_or_insert_with(HashMap::new);
        let mut keys_arr = Vec::new();
        for i in 0..keys.len() {
            if let Some(k) = keys.get(i) {
                let key_str = k.to_string();
                let is_default = keys_arr.is_empty();
                let entry = serde_json::json!({ "key": key_str, "is_default": is_default });
                keys_arr.push(entry);
            }
        }
        let entry = cfg.entry(name).or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("keys".into(), serde_json::Value::Array(keys_arr));
        }
        true
    }

    fn tool_definitions_json() -> Vec<serde_json::Value> {
        let tools = tools::all_tools();
        tools.iter().map(|t| {
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            for p in t.parameters() {
                let mut prop = serde_json::Map::new();
                prop.insert("type".into(), serde_json::Value::String(p.param_type.to_string()));
                prop.insert("description".into(), serde_json::Value::String(p.description.to_string()));
                properties.insert(p.name.to_string(), serde_json::Value::Object(prop));
                if p.required {
                    required.push(p.name.to_string());
                }
            }
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name(),
                    "description": t.description(),
                    "parameters": {
                        "type": "object",
                        "properties": properties,
                        "required": required,
                        "additionalProperties": false
                    }
                }
            })
        }).collect()
    }

    #[func]
    pub fn chat(
        &self,
        provider_name: GString,
        model: GString,
        messages_json: GString,
        api_key: GString,
        tools_enabled: bool,
    ) -> Dictionary {
        let prov = provider_name.to_string().to_lowercase();
        let base_url = match provider_base_url(&prov) {
            Some(u) => u,
            None => {
                godot_error!("Gchat: Unknown provider '{}'", prov);
                let mut d = Dictionary::new();
                let err_msg = format!("Unknown provider '{}'", prov);
                d.set("error", GString::from(&err_msg));
                return d;
            }
        };
        let key = api_key.to_string();
        if key.is_empty() {
            godot_error!("Gchat: No API key for '{}'", prov);
            let mut d = Dictionary::new();
            d.set("error", GString::from("No API key"));
            return d;
        }
        let url = format!("{}/chat/completions", base_url);
        let md = model.to_string();
        let msgs = messages_json.to_string();

        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                godot_error!("Gchat: Runtime: {}", e);
                let mut d = Dictionary::new();
                let err_msg = format!("Runtime: {}", e);
                d.set("error", GString::from(&err_msg));
                return d;
            }
        };

        let tool_list = if tools_enabled { Some(tools::all_tools()) } else { None };
        let tool_defs = if tools_enabled { Some(Self::tool_definitions_json()) } else { None };

        let result = rt.block_on(async {
            let client = reqwest::Client::builder()
                .user_agent("Gtool-Gchat/1.0")
                .build()
                .map_err(|e| (0u16, String::new(), format!("{:?}", e)))?;

            let mut messages: serde_json::Value = serde_json::from_str(&msgs)
                .unwrap_or(serde_json::Value::Array(vec![]));

            let max_iter = 10;
            for _iter in 0..max_iter {
                let mut body_obj = serde_json::json!({
                    "model": md,
                    "messages": messages,
                    "stream": false
                });
                if let Some(ref defs) = tool_defs {
                    body_obj["tools"] = serde_json::Value::Array(defs.clone());
                    body_obj["tool_choice"] = serde_json::Value::String("auto".into());
                }

                let body_str = serde_json::to_string(&body_obj).unwrap_or_default();
                let mut req = client.post(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .header("Content-Type", "application/json");

                if prov == "github" {
                    req = req.header("Content-Type", "application/json");
                }
                if prov == "gemini" {
                    req = req.header("x-goog-api-key", &key);
                }

                let resp = req.body(body_str).send().await
                    .map_err(|e| (0u16, String::new(), format!("HTTP: {}", e)))?;
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    let err = format!("HTTP {}: {}", status, text);
                    return Err((status.as_u16(), text, err));
                }

                let parsed: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err((status.as_u16(), text, format!("JSON parse: {}", e)));
                    }
                };

                let response_message = parsed.get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|c| c.get("message"))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                let tool_calls = response_message.get("tool_calls")
                    .and_then(|t| t.as_array())
                    .cloned()
                    .unwrap_or_default();

                // Append assistant message to conversation
                if let Some(arr) = messages.as_array_mut() {
                    arr.push(response_message.clone());
                }

                if tool_calls.is_empty() {
                    // No more tool calls — final response
                    let content = response_message.get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    return Ok((status.as_u16(), text, content.to_string()));
                }

                // Execute each tool call and append results
                for tc in &tool_calls {
                    let tool_name = tc.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let raw_args = tc.get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");
                    let call_id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");

                    let args_map: HashMap<String, String> = serde_json::from_str(raw_args).unwrap_or_default();
                    let tool_result = if let Some(ref tools_list) = tool_list {
                        if let Some(tool) = tools::find_tool(tools_list, tool_name) {
                            tool.call(args_map).await
                        } else {
                            Err(format!("Unknown tool: {}", tool_name))
                        }
                    } else {
                        Err("Tools not loaded".into())
                    };

                    let result_str = match tool_result {
                        Ok(r) => r,
                        Err(e) => format!("Error: {}", e),
                    };

                    if let Some(arr) = messages.as_array_mut() {
                        arr.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call_id,
                            "content": result_str
                        }));
                    }
                }
            }

            // Max iterations reached
            let last_text = messages.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("Max tool iterations reached");
            Err((0, String::new(), last_text.to_string()))
        });

        let mut dict = Dictionary::new();
        match result {
            Ok((code, raw_body, content)) => {
                dict.set("status", code as i64);
                dict.set("body", GString::from(&raw_body));
                dict.set("content", GString::from(&content));
                dict.set("role", GString::from("assistant"));
            }
            Err((code, raw_body, err_msg)) => {
                dict.set("status", code as i64);
                if !raw_body.is_empty() {
                    dict.set("body", GString::from(&raw_body));
                }
                if code == 0 {
                    dict.set("content", GString::from(&err_msg));
                } else {
                    dict.set("error", GString::from(&err_msg));
                }
            }
        }
        dict
    }
}
