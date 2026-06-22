mod cerebras;
mod cloudflare;
mod gemini;
mod github;
mod grok;
mod groq;
mod kilo;
mod llm7;
mod mistral;
mod nvidia;
mod openrouter;
mod persist;
mod session;
mod tools;
mod ui;
mod active_chat;

use std::io::{self, Write};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use async_trait::async_trait;
use edgequake_llm::{ChatMessage, ToolDefinition, LLMProvider, LLMResponse};
use tokio::sync::mpsc;
use tokio::io::{AsyncBufReadExt, BufReader};
use ui::{clear_screen, choose_provider, handle_config, handle_error, prompt_anyway};
use active_chat::{ChatEvent, spawn_chat_task, trim_buffer};

static PERSIST: OnceLock<Mutex<persist::Persist>> = OnceLock::new();

fn persist() -> &'static Mutex<persist::Persist> {
    PERSIST.get().expect("Persist not initialized (call init_persist first)")
}

fn prompt_password() -> Option<String> {
    print!("Enter config password (leave empty for no encryption): ");
    io::stdout().flush().unwrap();
    let mut pw = String::new();
    io::stdin().read_line(&mut pw).unwrap();
    let pw = pw.trim().to_string();
    if pw.is_empty() { None } else { Some(pw) }
}

pub fn init_persist() {
    PERSIST.set(Mutex::new(persist::Persist::load(prompt_password())))
        .unwrap_or_else(|_| panic!("Persist already initialized"));
}

#[async_trait]
pub trait ProviderTrait: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn model(&self) -> &str;
    fn supports_function_calling(&self) -> bool;
    async fn chat_stream(&self, prompt: &str) -> Result<(), Box<dyn std::error::Error>>;
    async fn chat(&self, messages: &[ChatMessage]) -> Result<LLMResponse, Box<dyn std::error::Error>>;
    async fn chat_with_tools(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<LLMResponse, Box<dyn std::error::Error>>;
}

pub type ProviderBox = Box<dyn ProviderTrait>;

pub enum Provider {
    OpenRouter(openrouter::OpenRouterChat),
    Grok(grok::GrokChat),
    Groq(groq::GroqChat),
    Nvidia(nvidia::NvidiaChat),
    Cerebras(cerebras::CerebrasChat),
    Mistral(mistral::MistralChat),
    Cloudflare(cloudflare::CloudflareChat),
    GitHub(github::GitHubChat),
    Gemini(gemini::GeminiChat),
    Llm7(llm7::Llm7Chat),
    Kilo(kilo::KiloChat),
}

impl Provider {
    pub async fn chat_stream(&self, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Provider::OpenRouter(c) => c.chat_stream(prompt).await,
            Provider::Grok(c) => c.chat_stream(prompt).await,
            Provider::Groq(c) => c.chat_stream(prompt).await,
            Provider::Nvidia(c) => c.chat_stream(prompt).await,
            Provider::Cerebras(c) => c.chat_stream(prompt).await,
            Provider::Mistral(c) => c.chat_stream(prompt).await,
            Provider::Cloudflare(c) => c.chat_stream(prompt).await,
            Provider::GitHub(c) => c.chat_stream(prompt).await,
            Provider::Gemini(c) => c.chat_stream(prompt).await,
            Provider::Llm7(c) => c.chat_stream(prompt).await,
            Provider::Kilo(c) => c.chat_stream(prompt).await,
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            Provider::OpenRouter(_) => "openrouter",
            Provider::Grok(_) => "xai",
            Provider::Groq(_) => "groq",
            Provider::Nvidia(_) => "nvidia",
            Provider::Cerebras(_) => "cerebras",
            Provider::Mistral(_) => "mistral",
            Provider::Cloudflare(_) => "cloudflare",
            Provider::GitHub(_) => "github",
            Provider::Gemini(_) => "gemini",
            Provider::Llm7(_) => "llm7",
            Provider::Kilo(_) => "kilo",
        }
    }

    pub fn model(&self) -> &str {
        self.as_provider().model()
    }

    pub fn supports_function_calling(&self) -> bool {
        match self {
            Provider::Gemini(_) => false,
            _ => self.as_provider().supports_function_calling(),
        }
    }

    pub fn as_provider(&self) -> &dyn LLMProvider {
        match self {
            Provider::OpenRouter(c) => c.as_provider(),
            Provider::Grok(c) => c.as_provider(),
            Provider::Groq(c) => c.as_provider(),
            Provider::Nvidia(c) => c.as_provider(),
            Provider::Cerebras(c) => c.as_provider(),
            Provider::Mistral(c) => c.as_provider(),
            Provider::Cloudflare(c) => c.as_provider(),
            Provider::GitHub(c) => c.as_provider(),
            Provider::Gemini(c) => c.as_provider(),
            Provider::Llm7(c) => c.as_provider(),
            Provider::Kilo(c) => c.as_provider(),
        }
    }

    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<LLMResponse, Box<dyn std::error::Error>> {
        Ok(self.as_provider().chat(messages, None).await?)
    }

    pub async fn chat_with_tools(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<LLMResponse, Box<dyn std::error::Error>> {
        Ok(self.as_provider().chat_with_tools(messages, tools, None, None).await?)
    }
}

#[async_trait]
impl ProviderTrait for Provider {
    fn provider_name(&self) -> &'static str { self.provider_name() }
    fn model(&self) -> &str { self.model() }
    fn supports_function_calling(&self) -> bool { self.supports_function_calling() }
    async fn chat_stream(&self, prompt: &str) -> Result<(), Box<dyn std::error::Error>> { self.chat_stream(prompt).await }
    async fn chat(&self, messages: &[ChatMessage]) -> Result<LLMResponse, Box<dyn std::error::Error>> { self.chat(messages).await }
    async fn chat_with_tools(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<LLMResponse, Box<dyn std::error::Error>> { self.chat_with_tools(messages, tools).await }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_persist();
    let mut sessions = session::SessionManager::new();
    let mut menu_requested = false;
    let (tx, mut rx) = mpsc::unbounded_channel::<ChatEvent>();
    let mut pending: HashSet<usize> = HashSet::new();
    let stdin = tokio::io::stdin();
    let mut stdin_lines = BufReader::new(stdin).lines();

    loop {
        if menu_requested || sessions.active_idx.is_none() || sessions.active().is_none() {
            menu_requested = false;
            clear_screen();

            if !sessions.sessions.is_empty() && sessions.active_idx.is_none() {
                println!("\nExisting sessions:");
                sessions.list();
            }

            let chat = match choose_provider() {
                Some(c) => c,
                None => return Ok(()),
            };

            let pname = chat.provider_name().to_string();
            let model = chat.model().to_string();
            let name = format!("{}/{}", pname, model);

            if persist().lock().unwrap().is_on_cooldown(&pname) {
                println!("Warning: {pname} is on cooldown (recent error).");
                if !prompt_anyway(&pname) {
                    continue;
                }
            }

            sessions.add(name, pname, model, Box::new(chat) as ProviderBox);
        }

        'inner: loop {
            {
                let s = sessions.active().unwrap();
                let tools_on = s.provider.as_ref().map(|p| p.supports_function_calling()).unwrap_or(false);
                let busy = if pending.contains(&s.id) { " [busy]" } else { "" };
                println!("\nActive: [{}] {} / {} ({} msgs | tools: {}){}",
                    s.id, s.provider_name, s.model, s.messages.len(), tools_on, busy);
            }
            print!("> ");
            std::io::stdout().flush()?;

            tokio::select! {
                Some(event) = rx.recv() => {
                    match event {
                        ChatEvent::Response { sid, content, prompt_tokens, completion_tokens, pname, provider } => {
                            pending.remove(&sid);
                            if let Some(a) = sessions.sessions.iter_mut().find(|s| s.id == sid) {
                                a.provider = provider;
                                a.messages.push(ChatMessage::assistant(&content));
                                trim_buffer(&mut a.messages);
                                persist().lock().unwrap().mark_healthy(&pname);
                                persist().lock().unwrap().track_tokens(&pname, prompt_tokens, completion_tokens);
                                let p = persist().lock().unwrap();
                                let calls = p.calls_in_window(&pname);
                                let key_idx = p.get_keys(&pname).iter().find(|(_, k)| k.is_default).map(|(i, _)| *i).unwrap_or(0);
                                let (pt, ct) = p.get_token_usage(&pname);
                                println!("\n{}", content);
                                println!("[Calls: {calls}/h | Key #{key_idx} | Tokens: {pt}in/{ct}out]");
                            }
                        }
                        ChatEvent::Error { sid, error, pname, provider } => {
                            pending.remove(&sid);
                            if let Some(a) = sessions.sessions.iter_mut().find(|s| s.id == sid) {
                                a.provider = provider;
                            }
                            handle_error(&pname, &error);
                        }
                    }
                }
                line = stdin_lines.next_line() => {
                    let input = match line? {
                        Some(s) => s.trim().to_string(),
                        None => return Ok(()),
                    };
                    if input.is_empty() { continue; }

                    match input.as_str() {
                        "quit" | "exit" => return Ok(()),
                        s if s.starts_with("menu") => { menu_requested = true; break 'inner; }
                        "" => continue,
                        "/help" => {
                            println!("\nCommands:");
                            println!("  /help                    This help");
                            println!("  /keys [<p>]              List keys");
                            println!("  /keys add [<p>] <key>    Add key");
                            println!("  /keys rm [<p>] <N>       Remove key");
                            println!("  /keys default [<p>] <N>  Set default key");
                            println!("  /config                  View config");
                            println!("  /nofree [<p>] <model>     Mark model as not free (blocked)");
                            println!("  /free [<p>] <model>      Unmark model as free");
                            println!("  /list                    List sessions");
                            println!("  /switch <N>              Switch session");
                            println!("  /close <N>               Close session");
                            println!("  /models [<p>]             List custom models");
                            println!("  /models add [<p>] <model>  Add custom model");
                            println!("  /models rm [<p>] <N>       Remove custom model");
                            println!("  menu                     New provider");
                            println!("  quit/exit                Exit");
                        }
                        s if s.starts_with("/keys") => {
                            let args: Vec<&str> = s.split_whitespace().collect();
                            let pname = sessions.active().map(|s| s.provider_name.clone()).unwrap_or_default();
                            match args.as_slice() {
                                ["/keys"] => {
                                    let p = persist().lock().unwrap();
                                    for (name, cfg) in &p.providers {
                                        println!("\n{name}:");
                                        for (i, k) in cfg.keys.iter().enumerate() {
                                            let def = if k.is_default { " (default)" } else { "" };
                                            let masked = if k.key.len() > 8 {
                                                format!("{}...{}", &k.key[..4], &k.key[k.key.len()-4..])
                                            } else {
                                                k.key.clone()
                                            };
                                            println!("  {i}. {masked}{def}");
                                        }
                                    }
                                }
                                ["/keys", provider] if *provider != "add" && *provider != "rm" && *provider != "default" => {
                                    let p = persist().lock().unwrap();
                                    for (i, k) in p.get_keys(provider) {
                                        let def = if k.is_default { " (default)" } else { "" };
                                        let masked = if k.key.len() > 8 {
                                            format!("{}...{}", &k.key[..4], &k.key[k.key.len()-4..])
                                        } else {
                                            k.key.clone()
                                        };
                                        println!("  {i}. {masked}{def}");
                                    }
                                }
                                ["/keys", "add", key] => {
                                    persist().lock().unwrap().add_key(&pname, (*key).to_string());
                                    println!("Key added to {pname}");
                                }
                                ["/keys", "add", provider, key] => {
                                    persist().lock().unwrap().add_key(provider, (*key).to_string());
                                    println!("Key added to {provider}");
                                }
                                ["/keys", "rm", idx] => {
                                    if let Ok(i) = idx.parse::<usize>() {
                                        if persist().lock().unwrap().remove_key(&pname, i) { println!("Removed"); }
                                        else { println!("Invalid"); }
                                    }
                                }
                                ["/keys", "rm", provider, idx] => {
                                    if let Ok(i) = idx.parse::<usize>() {
                                        if persist().lock().unwrap().remove_key(provider, i) { println!("Removed"); }
                                        else { println!("Invalid"); }
                                    }
                                }
                                ["/keys", "default", idx] => {
                                    if let Ok(i) = idx.parse::<usize>() {
                                        if persist().lock().unwrap().set_default_key(&pname, i) { println!("Default set"); }
                                        else { println!("Invalid"); }
                                    }
                                }
                                ["/keys", "default", provider, idx] => {
                                    if let Ok(i) = idx.parse::<usize>() {
                                        if persist().lock().unwrap().set_default_key(provider, i) { println!("Default set"); }
                                        else { println!("Invalid"); }
                                    }
                                }
                                _ => println!("Usage: /keys [provider] | /keys add [<provider>] <key> | /keys rm [<provider>] <N> | /keys default [<provider>] <N>"),
                            }
                        }
                        s if s.starts_with("/config") => { handle_config(s); }
                        s if s.starts_with("/nofree") => {
                            let args: Vec<&str> = s.split_whitespace().collect();
                            let pname = sessions.active().map(|s| s.provider_name.clone()).unwrap_or_default();
                            let cur_model = sessions.active().map(|s| s.model.clone());
                            match args.as_slice() {
                                ["/nofree"] => {
                                    if let Some(m) = cur_model {
                                        persist().lock().unwrap().block_model(&pname, &m);
                                        println!("{m} blocked on {pname}");
                                    } else { println!("No active session."); }
                                }
                                ["/nofree", model] => {
                                    persist().lock().unwrap().block_model(&pname, model);
                                    println!("{model} blocked on {pname}");
                                }
                                _ => println!("Usage: /nofree [<model>]"),
                            }
                        }
                        s if s.starts_with("/free") => {
                            let args: Vec<&str> = s.split_whitespace().collect();
                            let pname = sessions.active().map(|s| s.provider_name.clone()).unwrap_or_default();
                            let cur_model = sessions.active().map(|s| s.model.clone());
                            match args.as_slice() {
                                ["/free"] => {
                                    if let Some(m) = cur_model {
                                        persist().lock().unwrap().unblock_model(&pname, &m);
                                        println!("{m} unblocked on {pname}");
                                    } else { println!("No active session."); }
                                }
                                ["/free", model] => {
                                    persist().lock().unwrap().unblock_model(&pname, model);
                                    println!("{model} unblocked on {pname}");
                                }
                                _ => println!("Usage: /free [<model>]"),
                            }
                        }
                        "/list" => { sessions.list(); }
                        s if s.starts_with("/switch") => {
                            if let Some(id_str) = s.split_whitespace().nth(1) {
                                if let Ok(n) = id_str.parse::<usize>() {
                                    if sessions.switch(n) {
                                        println!("Switched to session {n}");
                                        continue;
                                    } else { println!("Session {n} not found"); }
                                }
                            }
                        }
                        s if s.trim().parse::<usize>().map_or(false, |n| n > 0) => {
                            let n = s.trim().parse::<usize>().unwrap();
                            if sessions.switch(n) {
                                println!("Switched to session {n}");
                                if let Some(a) = sessions.active() {
                                    let start = a.messages.len().saturating_sub(6);
                                    for m in &a.messages[start..] {
                                        let role = match m.role { edgequake_llm::ChatRole::User => "you", edgequake_llm::ChatRole::Assistant => "ai", _ => "sys" };
                                        let preview: String = m.content.chars().take(120).collect();
                                        println!("  [{role}] {preview}");
                                    }
                                }
                                continue;
                            } else { println!("Session {n} not found"); }
                        }
                        s if s.starts_with("/close") => {
                            let parts: Vec<&str> = s.split_whitespace().collect();
                            let n = parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                            let id = if n > 0 { n } else { sessions.active().map(|s| s.id).unwrap_or(0) };
                            if id > 0 && sessions.remove(id) {
                                println!("Session {id} closed");
                                if sessions.active().is_none() { break 'inner; }
                            }
                        }
                        s if s.starts_with("/models") => {
                            let args: Vec<&str> = s.split_whitespace().collect();
                            let pname = sessions.active().map(|s| s.provider_name.clone()).unwrap_or_default();
                            match args.as_slice() {
                                ["/models"] => {
                                    let m = persist().lock().unwrap().get_custom_models(&pname);
                                    if m.is_empty() { println!("No custom models for {pname}"); }
                                    else { for (i, mo) in m.iter().enumerate() { println!("  {i}. {mo}"); } }
                                }
                                ["/models", provider] if *provider != "add" && *provider != "rm" => {
                                    let m = persist().lock().unwrap().get_custom_models(provider);
                                    if m.is_empty() { println!("No custom models for {provider}"); }
                                    else { for (i, mo) in m.iter().enumerate() { println!("  {i}. {mo}"); } }
                                }
                                ["/models", "add", model] => {
                                    persist().lock().unwrap().add_custom_model(&pname, model);
                                    println!("Added to {pname}");
                                }
                                ["/models", "add", provider, model] => {
                                    persist().lock().unwrap().add_custom_model(provider, model);
                                    println!("Added to {provider}");
                                }
                                ["/models", "rm", idx] => {
                                    if let Ok(i) = idx.parse::<usize>() {
                                        if persist().lock().unwrap().remove_custom_model(&pname, i) { println!("Removed"); }
                                        else { println!("Invalid"); }
                                    }
                                }
                                ["/models", "rm", provider, idx] => {
                                    if let Ok(i) = idx.parse::<usize>() {
                                        if persist().lock().unwrap().remove_custom_model(provider, i) { println!("Removed"); }
                                        else { println!("Invalid"); }
                                    }
                                }
                                _ => println!("Usage: /models [<p>] | /models add [<p>] <model> | /models rm [<p>] <N>"),
                            }
                        }
                        _ => {
                            let sid = sessions.active().map(|s| s.id).unwrap_or(0);
                            let pname = sessions.active().map(|s| s.provider_name.clone()).unwrap_or_default();
                            persist().lock().unwrap().track_call(&pname);

                            let all_tools = tools::all_tools();
                            let tool_defs = tools::to_tool_definitions(&all_tools);

                            let msgs = {
                                let a = sessions.active().unwrap();
                                let mut m = a.messages.clone();
                                m.push(ChatMessage::user(&input));
                                m
                            };

                            if let Some(a) = sessions.active_mut() {
                                a.messages.push(ChatMessage::user(&input));
                                trim_buffer(&mut a.messages);
                            }

                            let provider = sessions.active_mut().and_then(|a| a.provider.take());
                            if pending.contains(&sid) {
                                println!("Session {sid} is busy, request queued.");
                                if let Some(a) = sessions.active_mut() { a.provider = provider; }
                            } else if let Some(p) = provider {
                                pending.insert(sid);
                                spawn_chat_task(tx.clone(), sid, &pname, p, msgs, tool_defs);
                            }
                        }
                    }
                }
            }
        }
    }
}
