use godot::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::gchat_core::*;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Gchat {
    config: Option<HashMap<String, serde_json::Value>>,
    password: String,
    context: ChatContext,
    stream_state: Option<Arc<Mutex<StreamState>>>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Gchat {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            config: None,
            password: String::new(),
            context: ChatContext::new(),
            stream_state: None,
            base,
        }
    }
}

#[godot_api]
impl Gchat {
    #[signal]
    fn message_appended(role: GString, content: GString);
    #[signal]
    fn stream_chunk(token: GString);
    #[signal]
    fn stream_reasoning_token(token: GString);
    #[signal]
    fn stream_finished(content: GString, reasoning: GString);
    #[signal]
    fn stream_error(error: GString);

    #[func]
    pub fn chat_stream(&mut self, provider_name: GString, model: GString, messages_json: GString, api_key: GString, tools_enabled: bool, api_format: GString, thinking_enabled: bool, thinking_budget: i64) -> bool {
        let prov = provider_name.to_string();
        let md = model.to_string();
        let msgs = messages_json.to_string();
        let key = api_key.to_string();
        if key.is_empty() {
            self.base_mut().emit_signal("stream_error", &[GString::from("No API key").to_variant()]);
            return false;
        }
        let fmt = ApiFormat::from_str(&api_format.to_string());
        let state = start_chat_stream(&prov, &md, &msgs, &key, tools_enabled, fmt, thinking_enabled, thinking_budget);
        self.stream_state = Some(state);
        true
    }

    #[func]
    pub fn poll_stream(&mut self) -> bool {
        let state = match self.stream_state.as_ref() {
            Some(s) => s,
            None => return false,
        };
        let (tokens, reason_tokens, finished, content, reasoning, error) = {
            let mut s = match state.lock() {
                Ok(g) => g,
                Err(_) => return false,
            };
            let t: Vec<String> = s.tokens.drain(..).collect();
            let rt: Vec<String> = s.reasoning_tokens.drain(..).collect();
            (t, rt, s.finished, s.content.clone(), s.reasoning.clone(), s.error.clone())
        };
        for token in &tokens {
            self.base_mut().emit_signal("stream_chunk", &[GString::from(token).to_variant()]);
        }
        for token in &reason_tokens {
            self.base_mut().emit_signal("stream_reasoning_token", &[GString::from(token).to_variant()]);
        }
        if !error.is_empty() {
            self.stream_state = None;
            self.base_mut().emit_signal("stream_error", &[GString::from(&error).to_variant()]);
            return true;
        }
        if finished {
            self.stream_state = None;
            self.base_mut().emit_signal("stream_finished", &[GString::from(&content).to_variant(), GString::from(&reasoning).to_variant()]);
        }
        true
    }

    #[func]
    pub fn is_streaming(&self) -> bool {
        self.stream_state.is_some()
    }

    #[func]
    pub fn append_context(&mut self, role_json: GString) {
        self.context.append_json(&role_json.to_string());
    }

    #[func]
    pub fn get_context(&self) -> GString {
        GString::from(&self.context.get_json())
    }

    #[func]
    pub fn clear_context(&mut self) {
        self.context.clear();
    }

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
            match decrypt_config_json(data_str, &pw) {
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
        let data_b64 = match encrypt_config_json(&plaintext, &password.to_string()) {
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
    pub fn add_provider(&mut self, name: GString) -> bool {
        let cfg = self.config.get_or_insert_with(HashMap::new);
        let prov_name = name.to_string();
        if cfg.contains_key(&prov_name) {
            return false;
        }
        cfg.insert(prov_name, serde_json::json!({"keys": []}));
        true
    }

    #[func]
    pub fn remove_provider(&mut self, name: GString) -> bool {
        if let Some(ref mut cfg) = self.config {
            cfg.remove(&name.to_string()).is_some()
        } else {
            false
        }
    }

    #[func]
    pub fn add_key(&mut self, provider: GString, key: GString) -> bool {
        let cfg = self.config.get_or_insert_with(HashMap::new);
        let prov_name = provider.to_string();
        let entry = cfg.entry(prov_name).or_insert_with(|| serde_json::json!({"keys": []}));
        if let Some(obj) = entry.as_object_mut() {
            let keys = obj.entry("keys").or_insert_with(|| serde_json::Value::Array(vec![]));
            if let Some(arr) = keys.as_array_mut() {
                let is_default = arr.is_empty();
                arr.push(serde_json::json!({"key": key.to_string(), "is_default": is_default}));
                return true;
            }
        }
        false
    }

    #[func]
    pub fn remove_key(&mut self, provider: GString, index: i64) -> bool {
        let cfg = match self.config.as_mut() {
            Some(c) => c,
            None => return false,
        };
        let prov_name = provider.to_string();
        let entry = match cfg.get_mut(&prov_name) {
            Some(e) => e,
            None => return false,
        };
        if let Some(obj) = entry.as_object_mut() {
            if let Some(keys) = obj.get_mut("keys").and_then(|k| k.as_array_mut()) {
                if index >= 0 && (index as usize) < keys.len() {
                    keys.remove(index as usize);
                    return true;
                }
            }
        }
        false
    }

    #[func]
    pub fn get_provider_model(&self, provider: GString) -> GString {
        let name = provider.to_string();
        if let Some(ref cfg) = self.config {
            if let Some(data) = cfg.get(&name) {
                if let Some(model) = data.get("model").and_then(|m| m.as_str()) {
                    return GString::from(model);
                }
            }
        }
        GString::new()
    }

    #[func]
    pub fn set_provider_model(&mut self, provider: GString, model: GString) -> bool {
        let name = provider.to_string();
        let cfg = self.config.get_or_insert_with(HashMap::new);
        let entry = cfg.entry(name).or_insert_with(|| serde_json::json!({"keys": []}));
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("model".into(), serde_json::Value::String(model.to_string()));
            return true;
        }
        false
    }

    #[func]
    pub fn get_custom_models(&self, provider: GString) -> Array<GString> {
        let mut arr = Array::new();
        let name = provider.to_string();
        if let Some(ref cfg) = self.config {
            if let Some(data) = cfg.get(&name) {
                if let Some(models) = data.get("custom_models").and_then(|v| v.as_array()) {
                    for m in models {
                        if let Some(s) = m.as_str() {
                            arr.push(&GString::from(s));
                        }
                    }
                }
            }
        }
        arr
    }

    #[func]
    pub fn add_custom_model(&mut self, provider: GString, model: GString) -> bool {
        let name = provider.to_string();
        let cfg = self.config.get_or_insert_with(HashMap::new);
        let entry = cfg.entry(name).or_insert_with(|| serde_json::json!({"keys": []}));
        if let Some(obj) = entry.as_object_mut() {
            let models = obj.entry("custom_models").or_insert_with(|| serde_json::Value::Array(vec![]));
            if let Some(arr) = models.as_array_mut() {
                let model_str = model.to_string();
                if !arr.iter().any(|m| m.as_str() == Some(&model_str)) {
                    arr.push(serde_json::Value::String(model_str));
                    return true;
                }
            }
        }
        false
    }

    #[func]
    pub fn remove_custom_model(&mut self, provider: GString, index: i64) -> bool {
        let cfg = match self.config.as_mut() {
            Some(c) => c,
            None => return false,
        };
        let prov_name = provider.to_string();
        let entry = match cfg.get_mut(&prov_name) {
            Some(e) => e,
            None => return false,
        };
        if let Some(obj) = entry.as_object_mut() {
            if let Some(models) = obj.get_mut("custom_models").and_then(|k| k.as_array_mut()) {
                if index >= 0 && (index as usize) < models.len() {
                    models.remove(index as usize);
                    return true;
                }
            }
        }
        false
    }

    #[func]
    pub fn set_default_key(&mut self, provider: GString, index: i64) -> bool {
        let cfg = match self.config.as_mut() {
            Some(c) => c,
            None => return false,
        };
        let prov_name = provider.to_string();
        let entry = match cfg.get_mut(&prov_name) {
            Some(e) => e,
            None => return false,
        };
        if let Some(obj) = entry.as_object_mut() {
            if let Some(keys) = obj.get_mut("keys").and_then(|k| k.as_array_mut()) {
                if index < 0 || (index as usize) >= keys.len() {
                    return false;
                }
                for (i, k) in keys.iter_mut().enumerate() {
                    if let Some(kobj) = k.as_object_mut() {
                        kobj.insert("is_default".into(), serde_json::Value::Bool(i == index as usize));
                    }
                }
                return true;
            }
        }
        false
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

    fn emit_new_messages(&mut self, initial_count: usize, full_messages_json: &str) {
        if let Ok(full) = serde_json::from_str::<serde_json::Value>(full_messages_json) {
            if let Some(arr) = full.as_array() {
                for i in initial_count..arr.len() {
                    if let Some(msg) = arr.get(i) {
                        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        self.base_mut().emit_signal("message_appended",
                            &[role.to_variant(), content.to_variant()]);
                    }
                }
            }
        }
    }

    #[func]
    pub fn chat(
        &mut self,
        provider_name: GString,
        model: GString,
        messages_json: GString,
        api_key: GString,
        tools_enabled: bool,
        api_format: GString,
        thinking_enabled: bool,
        thinking_budget: i64,
    ) -> Dictionary {
        let prov = provider_name.to_string();
        let md = model.to_string();
        let msgs = messages_json.to_string();
        let key = api_key.to_string();
        let fmt = ApiFormat::from_str(&api_format.to_string());

        let initial_msg_count = serde_json::from_str::<serde_json::Value>(&msgs)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .map(|a| a.len())
            .unwrap_or(0);

        let result = perform_chat(&prov, &md, &msgs, &key, tools_enabled, fmt, thinking_enabled, thinking_budget);

        if !result.full_messages.is_empty() {
            self.emit_new_messages(initial_msg_count, &result.full_messages);
        }

        let mut dict = Dictionary::new();
        if result.is_error() {
            dict.set("status", result.status as i64);
            if !result.raw_body.is_empty() {
                dict.set("body", GString::from(&result.raw_body));
            }
            if !result.reasoning.is_empty() {
                dict.set("reasoning", GString::from(&result.reasoning));
            }
            if result.status == 0 {
                dict.set("content", GString::from(&result.error));
            } else {
                dict.set("error", GString::from(&result.error));
            }
        } else {
            dict.set("status", result.status as i64);
            dict.set("body", GString::from(&result.raw_body));
            dict.set("content", GString::from(&result.content));
            dict.set("reasoning", GString::from(&result.reasoning));
            dict.set("role", GString::from("assistant"));
        }
        dict
    }
}
