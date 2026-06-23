use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
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

pub fn decrypt_config_json(data: &str, password: &str) -> Result<String, String> {
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

pub fn encrypt_config_json(plaintext: &str, password: &str) -> Result<String, String> {
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

pub fn provider_base_url(name: &str) -> Option<&'static str> {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiFormat {
    ChatCompletions,
    Messages,
}

impl ApiFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "messages" => Self::Messages,
            _ => Self::ChatCompletions,
        }
    }

    pub fn endpoint(&self, base_url: &str) -> String {
        match self {
            Self::ChatCompletions => format!("{}/chat/completions", base_url),
            Self::Messages => format!("{}/messages", base_url),
        }
    }
}

fn anthropic_messages_to_openai(anthropic_msgs: &serde_json::Value) -> serde_json::Value {
    if let Some(arr) = anthropic_msgs.as_array() {
        let mut out = Vec::new();
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = if let Some(content_arr) = msg.get("content").and_then(|c| c.as_array()) {
                let mut text_parts = Vec::new();
                for block in content_arr {
                    if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                        match block_type {
                            "text" => {
                                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                    text_parts.push(t);
                                }
                            }
                            "tool_use" | "tool_result" => {}
                            "thinking" => {}
                            _ => {}
                        }
                    }
                }
                text_parts.join("")
            } else {
                msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string()
            };
            let mut out_msg = serde_json::json!({"role": role, "content": content});
            if let Some(tc) = msg.get("tool_calls") {
                out_msg["tool_calls"] = tc.clone();
            }
            out.push(out_msg);
        }
        serde_json::Value::Array(out)
    } else {
        anthropic_msgs.clone()
    }
}

fn openai_messages_to_anthropic(openai_msgs: &serde_json::Value) -> serde_json::Value {
    if let Some(arr) = openai_msgs.as_array() {
        let mut out = Vec::new();
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content_val = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let tool_calls = msg.get("tool_calls");
            let mut content_blocks: Vec<serde_json::Value> = Vec::new();
            if role == "assistant" && tool_calls.is_some() {
                if !content_val.is_empty() {
                    content_blocks.push(serde_json::json!({"type": "text", "text": content_val}));
                }
                if let Some(tc_arr) = tool_calls.and_then(|t| t.as_array()) {
                    for tc in tc_arr {
                        let name = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                        let raw_args = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
                        let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                        let args_val: serde_json::Value = serde_json::from_str(raw_args).unwrap_or(serde_json::Value::Null);
                        content_blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": args_val
                        }));
                    }
                }
            } else if role == "tool" {
                let tool_use_id = msg.get("tool_call_id").and_then(|i| i.as_str()).unwrap_or("");
                content_blocks.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": content_val
                }));
            } else {
                content_blocks.push(serde_json::json!({"type": "text", "text": content_val}));
            }
            let out_msg = serde_json::json!({"role": role, "content": content_blocks});
            out.push(out_msg);
        }
        serde_json::Value::Array(out)
    } else {
        openai_msgs.clone()
    }
}

fn build_request_body(
    format: ApiFormat,
    model: &str,
    messages: &serde_json::Value,
    stream: bool,
    tool_defs: &Option<Vec<serde_json::Value>>,
    thinking_enabled: bool,
    thinking_budget: i64,
) -> serde_json::Value {
    match format {
        ApiFormat::ChatCompletions => {
            let mut body = serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": stream
            });
            if let Some(defs) = tool_defs {
                body["tools"] = serde_json::Value::Array(defs.clone());
                body["tool_choice"] = serde_json::Value::String("auto".into());
            }
            body
        }
        ApiFormat::Messages => {
            let anthropic_msgs = openai_messages_to_anthropic(messages);
            let mut body = serde_json::json!({
                "model": model,
                "messages": anthropic_msgs,
                "stream": stream
            });
            if let Some(defs) = tool_defs {
                body["tools"] = serde_json::Value::Array(defs.clone());
                body["tool_choice"] = serde_json::Value::Object({
                    let mut m = serde_json::Map::new();
                    m.insert("type".into(), serde_json::Value::String("auto".into()));
                    m
                });
            }
            if thinking_enabled {
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": thinking_budget
                });
            }
            body
        }
    }
}

fn parse_openai_response(body: &serde_json::Value) -> (String, String, Vec<serde_json::Value>) {
    let content = body.get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("").to_string();
    let reasoning = body.get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("reasoning_content").or_else(|| m.get("reasoning")))
        .and_then(|c| c.as_str())
        .unwrap_or("").to_string();
    let tool_calls = body.get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("tool_calls"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    (content, reasoning, tool_calls)
}

fn parse_anthropic_response(body: &serde_json::Value) -> (String, String, Vec<serde_json::Value>) {
    let mut content_text = String::new();
    let mut reasoning_text = String::new();
    let mut tool_calls = Vec::new();
    if let Some(content_arr) = body.get("content").and_then(|c| c.as_array()) {
        for block in content_arr {
            match block.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                "text" => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        content_text.push_str(t);
                    }
                }
                "thinking" => {
                    if let Some(t) = block.get("thinking").and_then(|t| t.as_str()) {
                        reasoning_text.push_str(t);
                    }
                }
                "tool_use" => {
                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let input = block.get("input").map(|i| i.to_string()).unwrap_or_default();
                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    tool_calls.push(serde_json::json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": input
                        }
                    }));
                }
                _ => {}
            }
        }
    }
    (content_text, reasoning_text, tool_calls)
}

fn response_message_to_json(
    format: ApiFormat,
    body: &serde_json::Value,
) -> serde_json::Value {
    match format {
        ApiFormat::ChatCompletions => {
            body.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("message"))
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        }
        ApiFormat::Messages => {
            let (content, reasoning, tool_calls) = parse_anthropic_response(body);
            let role = body.get("role").and_then(|r| r.as_str()).unwrap_or("assistant");
            let mut msg = serde_json::json!({
                "role": role,
                "content": content,
            });
            if !reasoning.is_empty() {
                msg["reasoning_content"] = serde_json::Value::String(reasoning);
            }
            if !tool_calls.is_empty() {
                msg["tool_calls"] = serde_json::Value::Array(tool_calls);
            }
            msg
        }
    }
}

fn tool_calls_from_message(
    format: ApiFormat,
    response_message: &serde_json::Value,
) -> Vec<serde_json::Value> {
    match format {
        ApiFormat::ChatCompletions => {
            response_message.get("tool_calls")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default()
        }
        ApiFormat::Messages => {
            response_message.get("tool_calls")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default()
        }
    }
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

#[derive(Default)]
pub struct ChatContext {
    messages: Vec<serde_json::Value>,
}

impl ChatContext {
    pub fn new() -> Self {
        Self { messages: Vec::new() }
    }

    pub fn append_json(&mut self, role_json: &str) {
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(role_json) {
            self.messages.push(msg);
        }
    }

    pub fn push_message(&mut self, msg: serde_json::Value) {
        self.messages.push(msg);
    }

    pub fn get_json(&self) -> String {
        serde_json::to_string(&self.messages).unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn messages(&self) -> &Vec<serde_json::Value> {
        &self.messages
    }
}

#[derive(Debug, Clone)]
pub struct ChatResult {
    pub status: u16,
    pub raw_body: String,
    pub content: String,
    pub reasoning: String,
    pub error: String,
    pub full_messages: String,
}

impl ChatResult {
    pub fn is_error(&self) -> bool {
        !self.error.is_empty()
    }

    pub fn ok(status: u16, raw_body: String, content: String, reasoning: String, full_messages: String) -> Self {
        Self { status, raw_body, content, reasoning, error: String::new(), full_messages }
    }

    pub fn err(status: u16, raw_body: String, reasoning: String, error: String) -> Self {
        Self { status, raw_body, content: String::new(), reasoning, error, full_messages: String::new() }
    }
}

pub fn perform_chat(
    provider: &str,
    model: &str,
    messages_json: &str,
    api_key: &str,
    tools_enabled: bool,
    format: ApiFormat,
    thinking_enabled: bool,
    thinking_budget: i64,
) -> ChatResult {
    let base_url = match provider_base_url(provider) {
        Some(u) => u,
        None => return ChatResult::err(0, String::new(), String::new(),
            format!("Unknown provider '{}'", provider)),
    };
    if api_key.is_empty() {
        return ChatResult::err(0, String::new(), String::new(), "No API key".into());
    }
    let url = format.endpoint(&base_url);
    let md = model.to_string();
    let msgs = messages_json.to_string();
    let prov = provider.to_string();

    let rt = match Runtime::new() {
        Ok(r) => r,
        Err(e) => return ChatResult::err(0, String::new(), String::new(),
            format!("Runtime: {}", e)),
    };

    let tool_list = if tools_enabled { Some(tools::all_tools()) } else { None };
    let tool_defs = if tools_enabled { Some(tool_definitions_json()) } else { None };

    rt.block_on(async {
        let client = match reqwest::Client::builder()
            .user_agent("Gtool-Gchat/1.0")
            .build()
        {
            Ok(c) => c,
            Err(e) => return ChatResult::err(0, String::new(), String::new(), format!("{:?}", e)),
        };

        let mut messages: serde_json::Value = match serde_json::from_str(&msgs) {
            Ok(v) => v,
            Err(_) => serde_json::Value::Array(vec![]),
        };

        let max_iter = 10;
        for _iter in 0..max_iter {
            let body_obj = build_request_body(format, &md, &messages, false, &tool_defs, thinking_enabled, thinking_budget);
            let body_str = serde_json::to_string(&body_obj).unwrap_or_default();
            let mut req = client.post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json");

            if prov == "github" {
                req = req.header("Content-Type", "application/json");
            }
            if prov == "gemini" {
                req = req.header("x-goog-api-key", api_key);
            }

            let resp = match req.body(body_str).send().await {
                Ok(r) => r,
                Err(e) => return ChatResult::err(0, String::new(), String::new(),
                    format!("HTTP: {}", e)),
            };
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                let err = format!("HTTP {}: {}", status, text);
                return ChatResult::err(status.as_u16(), text, String::new(), err);
            }

            let parsed: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return ChatResult::err(status.as_u16(), text, String::new(),
                    format!("JSON parse: {}", e)),
            };

            let response_message = response_message_to_json(format, &parsed);
            let tool_calls = tool_calls_from_message(format, &response_message);

            if let Some(arr) = messages.as_array_mut() {
                arr.push(response_message.clone());
            }

            if tool_calls.is_empty() {
                let (content, reasoning, _) = match format {
                    ApiFormat::ChatCompletions => parse_openai_response(&parsed),
                    ApiFormat::Messages => parse_anthropic_response(&parsed),
                };
                let full_msgs = serde_json::to_string(&messages).unwrap_or_default();
                return ChatResult::ok(status.as_u16(), text, content, reasoning, full_msgs);
            }

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

        ChatResult::err(0, String::new(), String::new(), "Max tool iterations reached".into())
    })
}

pub struct StreamState {
    pub tokens: VecDeque<String>,
    pub reasoning_tokens: VecDeque<String>,
    pub finished: bool,
    pub content: String,
    pub reasoning: String,
    pub error: String,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            tokens: VecDeque::new(),
            reasoning_tokens: VecDeque::new(),
            finished: false,
            content: String::new(),
            reasoning: String::new(),
            error: String::new(),
        }
    }
}

pub fn start_chat_stream(
    provider: &str,
    model: &str,
    messages_json: &str,
    api_key: &str,
    tools_enabled: bool,
    format: ApiFormat,
    thinking_enabled: bool,
    thinking_budget: i64,
) -> Arc<Mutex<StreamState>> {
    let state = Arc::new(Mutex::new(StreamState::new()));
    let state_clone = state.clone();

    let prov = provider.to_string();
    let md = model.to_string();
    let msgs = messages_json.to_string();
    let key = api_key.to_string();

    std::thread::spawn(move || {
        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                let mut s = state_clone.lock().unwrap();
                s.error = format!("Runtime: {}", e);
                s.finished = true;
                return;
            }
        };

        rt.block_on(async move {
            let base_url = match provider_base_url(&prov) {
                Some(u) => u,
                None => {
                    let mut s = state_clone.lock().unwrap();
                    s.error = format!("Unknown provider '{}'", prov);
                    s.finished = true;
                    return;
                }
            };
            let url = format.endpoint(&base_url);

            let client = match reqwest::Client::builder()
                .user_agent("Gtool-Gchat/1.0")
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    let mut s = state_clone.lock().unwrap();
                    s.error = format!("{:?}", e);
                    s.finished = true;
                    return;
                }
            };

            let tool_defs = if tools_enabled { Some(tool_definitions_json()) } else { None };
            let messages: serde_json::Value = match serde_json::from_str(&msgs) {
                Ok(v) => v,
                Err(_) => serde_json::Value::Array(vec![]),
            };

            let body_obj = build_request_body(format, &md, &messages, true, &tool_defs, thinking_enabled, thinking_budget);
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

            let mut resp = match req.body(body_str).send().await {
                Ok(r) => r,
                Err(e) => {
                    let mut s = state_clone.lock().unwrap();
                    s.error = format!("HTTP: {}", e);
                    s.finished = true;
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let mut s = state_clone.lock().unwrap();
                s.error = format!("HTTP {}: {}", status, text);
                s.finished = true;
                return;
            }

            let mut buffer = String::new();
            let mut accumulated = String::new();
            let mut reasoning_acc = String::new();

            match format {
                ApiFormat::ChatCompletions => {
                    while let Some(chunk) = match resp.chunk().await {
                        Ok(Some(c)) => Some(c),
                        Ok(None) => None,
                        Err(e) => {
                            let mut s = state_clone.lock().unwrap();
                            s.error = format!("Stream error: {}", e);
                            s.finished = true;
                            return;
                        }
                    } {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&chunk_str);
                        while let Some(line_end) = buffer.find('\n') {
                            let line = buffer[..line_end].trim_end().to_string();
                            buffer = buffer[line_end + 1..].to_string();
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    let mut s = state_clone.lock().unwrap();
                                    s.content = accumulated.clone();
                                    s.reasoning = reasoning_acc.clone();
                                    s.finished = true;
                                    return;
                                }
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array()) {
                                        if let Some(choice) = choices.first() {
                                            if let Some(delta) = choice.get("delta") {
                                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                    accumulated.push_str(content);
                                                    let mut s = state_clone.lock().unwrap();
                                                    s.tokens.push_back(content.to_string());
                                                }
                                                if let Some(reasoning) = delta.get("reasoning_content")
                                                    .or_else(|| delta.get("reasoning"))
                                                    .and_then(|c| c.as_str())
                                                {
                                                    reasoning_acc.push_str(reasoning);
                                                    let mut s = state_clone.lock().unwrap();
                                                    s.reasoning_tokens.push_back(reasoning.to_string());
                                                }
                                            }
                                            if let Some(finish) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                                                if finish != "null" && !finish.is_empty() {
                                                    let mut s = state_clone.lock().unwrap();
                                                    s.content = accumulated.clone();
                                                    s.reasoning = reasoning_acc.clone();
                                                    s.finished = true;
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ApiFormat::Messages => {
                    while let Some(chunk) = match resp.chunk().await {
                        Ok(Some(c)) => Some(c),
                        Ok(None) => None,
                        Err(e) => {
                            let mut s = state_clone.lock().unwrap();
                            s.error = format!("Stream error: {}", e);
                            s.finished = true;
                            return;
                        }
                    } {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&chunk_str);
                    let mut _in_thinking = false;
                    while let Some(line_end) = buffer.find('\n') {
                        let line = buffer[..line_end].trim_end().to_string();
                        buffer = buffer[line_end + 1..].to_string();
                        if let Some(data) = line.strip_prefix("data: ") {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                                let event_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                match event_type {
                                        "content_block_start" => {
                                            if let Some(block) = parsed.get("content_block") {
                                                match block.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                                                    "thinking" => _in_thinking = true,
                                                    "text" => _in_thinking = false,
                                                    _ => {}
                                                }
                                            }
                                        }
                                        "content_block_delta" => {
                                            if let Some(delta) = parsed.get("delta") {
                                                match delta.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                                                    "text_delta" => {
                                                        if let Some(t) = delta.get("text").and_then(|t| t.as_str()) {
                                                            accumulated.push_str(t);
                                                            let mut s = state_clone.lock().unwrap();
                                                            s.tokens.push_back(t.to_string());
                                                        }
                                                    }
                                                    "thinking_delta" => {
                                                        if let Some(t) = delta.get("thinking").and_then(|t| t.as_str()) {
                                                            reasoning_acc.push_str(t);
                                                            let mut s = state_clone.lock().unwrap();
                                                            s.reasoning_tokens.push_back(t.to_string());
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        "message_stop" => {
                                            let mut s = state_clone.lock().unwrap();
                                            s.content = accumulated.clone();
                                            s.reasoning = reasoning_acc.clone();
                                            s.finished = true;
                                            return;
                                        }
                                        "error" => {
                                            let err_msg = parsed.get("error").and_then(|e| e.as_str()).unwrap_or("stream error");
                                            let mut s = state_clone.lock().unwrap();
                                            s.error = err_msg.to_string();
                                            s.finished = true;
                                            return;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut s = state_clone.lock().unwrap();
            s.content = accumulated;
            s.reasoning = reasoning_acc;
            s.finished = true;
        });
    });

    state
}
