use crate::{persist, Provider, ProviderBox};
use std::io::{self, Write};

pub fn get_or_prompt_key(provider: &str, prompt_msg: &str) -> String {
    let mut p = persist().lock().unwrap();
    if let Some(key) = p.get_key(provider) {
        return key;
    }
    print!("{prompt_msg}");
    io::stdout().flush().unwrap();
    let mut key = String::new();
    io::stdin().read_line(&mut key).unwrap();
    let key = key.trim().to_string();
    p.set_key(provider, key.clone());
    key
}

pub fn choose_model(prompt: &str, default: &str) -> String {
    print!("{prompt} (default: {default}): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_string();
    if input.is_empty() { default.to_string() } else { input }
}

pub fn pick_from_list(models: &[String], label: &str, provider: &str) -> String {
    let p = persist().lock().unwrap();
    let blocked = p.get_blocked_models(provider);
    let custom = p.get_custom_models(provider);
    let all: Vec<String> = models.iter().cloned().chain(custom).collect();
    drop(p);
    loop {
        println!("\n{label}:");
        for (i, m) in all.iter().enumerate() {
            let mark = if blocked.contains(m) { " [BLOCKED]" } else { "" };
            println!("  {}. {m}{mark}", i + 1);
        }
        print!("Select (1-{}): ", all.len());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if let Ok(n) = input.trim().parse::<usize>() {
            if n >= 1 && n <= all.len() {
                return all[n - 1].clone();
            }
        }
        println!("Invalid.");
    }
}

pub fn provider_status(name: &str) -> String {
    let p = persist().lock().unwrap();
    let keys = p.get_keys(name);
    if keys.is_empty() { return String::new(); }
    let total = keys.len();
    let def = keys.iter().find(|(_, k)| k.is_default).map(|(i, _)| i).unwrap_or(&0);
    format!(" [{total} key(s), default #{def}]")
}

pub fn clear_screen() {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd").args(["/c", "cls"]).status().ok();
    }
    #[cfg(not(windows))]
    {
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().ok();
    }
}

pub fn choose_provider() -> Option<Provider> {
    clear_screen();
    loop {
        print!(
            "\nSelect provider:\n  \
               1. OpenRouter{}\n  \
               2. xAI Grok{}\n  \
               3. Groq{}\n  \
               4. NVIDIA{}\n  \
               5. Cerebras{}\n  \
               6. Mistral{}\n  \
               7. Cloudflare AI Gateway{}\n  \
               8. GitHub Models{}\n  \
               9. Google Gemini{}\n  \
               10. LLM7.io{}\n  \
               11. Kilo Code{}\n  \
               12. Only free models\n\
            Choice (1-12, 0 to exit): ",
            provider_status("openrouter"),
            provider_status("xai"),
            provider_status("groq"),
            provider_status("nvidia"),
            provider_status("cerebras"),
            provider_status("mistral"),
            provider_status("cloudflare"),
            provider_status("github"),
            provider_status("gemini"),
            provider_status("llm7"),
            provider_status("kilo"),
        );
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim() {
            "0" => return None,
            "1" => {
                let key = get_or_prompt_key("openrouter", "Enter your OpenRouter API key: ");
                let model = choose_model("Model", "openrouter/free");
                match crate::openrouter::OpenRouterChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::OpenRouter(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "2" => {
                let key = get_or_prompt_key("xai", "Enter your xAI API key: ");
                let model = choose_model("Model", "grok-4");
                match crate::grok::GrokChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Grok(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "3" => {
                let key = get_or_prompt_key("groq", "Enter your Groq API key: ");
                let models = vec![
                    "llama-3.3-70b-versatile".to_string(),
                    "llama-3.1-8b-instant".to_string(),
                    "mixtral-8x7b-32768".to_string(),
                    "gemma2-9b-it".to_string(),
                ];
                let model = pick_from_list(&models, "Groq free models", "groq");
                match crate::groq::GroqChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Groq(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "4" => {
                let key = get_or_prompt_key("nvidia", "Enter your NVIDIA API key (nvapi-...): ");
                let models = vec![
                    "minimax/minimax-m3".to_string(),
                    "nvidia/nemotron-3-ultra-550b-a55b".to_string(),
                ];
                let model = pick_from_list(&models, "NVIDIA models", "nvidia");
                match crate::nvidia::NvidiaChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Nvidia(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "5" => {
                let key = get_or_prompt_key("cerebras", "Enter your Cerebras API key: ");
                let models = vec![
                    "zai-glm-4.7".to_string(),
                    "gpt-oss-120b".to_string(),
                ];
                let model = pick_from_list(&models, "Cerebras models", "cerebras");
                match crate::cerebras::CerebrasChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Cerebras(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "6" => {
                let key = get_or_prompt_key("mistral", "Enter your Mistral API key: ");
                let models = vec![
                    "mistral-large-latest".to_string(),
                    "mistral-medium-latest".to_string(),
                    "mistral-small-latest".to_string(),
                    "ministral-3b-latest".to_string(),
                    "ministral-8b-latest".to_string(),
                ];
                let model = pick_from_list(&models, "Mistral models", "mistral");
                match crate::mistral::MistralChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Mistral(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "7" => {
                let token;
                let account_id;
                {
                    let p = persist().lock().unwrap();
                    token = p.get_key("cloudflare").unwrap_or_default();
                    account_id = p.get_metadata("cloudflare", "account_id").unwrap_or_default();
                }
                let (token, account_id) = if token.is_empty() || account_id.is_empty() {
                    crate::cloudflare::prompt_api_token()
                } else {
                    (token, account_id)
                };
                {
                    let mut p = persist().lock().unwrap();
                    p.set_key("cloudflare", token.clone());
                    p.set_metadata("cloudflare", "account_id".to_string(), account_id.clone());
                }
                let models = vec![
                    "openai/gpt-4.1".to_string(),
                    "anthropic/claude-sonnet-4".to_string(),
                    "google/gemini-3-flash".to_string(),
                    "@cf/moonshotai/kimi-k2.6".to_string(),
                ];
                let model = pick_from_list(&models, "Cloudflare AI Gateway models", "cloudflare");
                match crate::cloudflare::CloudflareChat::new(&token, &account_id, &model) {
                    Ok(c) => return Some(Provider::Cloudflare(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "8" => {
                let key = get_or_prompt_key("github", "Enter your GitHub PAT (from github.com/settings/tokens): ");
                let models = vec![
                    "DeepSeek-V3-0324".to_string(),
                    "Phi-4-reasoning".to_string(),
                    "Phi-4-multimodal-instruct".to_string(),
                    "Phi-4-mini-instruct".to_string(),
                    "Phi-4-mini-reasoning".to_string(),
                ];
                let model = pick_from_list(&models, "GitHub Models", "github");
                match crate::github::GitHubChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::GitHub(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "9" => {
                let key = get_or_prompt_key("gemini", "Enter your Google Gemini API key (from aistudio.google.com): ");
                let models = vec![
                    "gemini-3.5-flash".to_string(),
                    "gemini-3.1-flash-lite".to_string(),
                    "gemini-3-flash-preview".to_string(),
                ];
                let model = pick_from_list(&models, "Gemini models", "gemini");
                match crate::gemini::GeminiChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Gemini(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "10" => {
                let key = get_or_prompt_key("llm7", "Enter your LLM7.io token (from token.llm7.io): ");
                let models = vec![
                    "deepseek-r1-0528".to_string(),
                    "deepseek-v3-0324".to_string(),
                    "mistral-small-3.1-24b".to_string(),
                    "qwen2.5-coder-32b".to_string(),
                    "gpt-4o-mini".to_string(),
                    "gemini-2.5-flash-lite".to_string(),
                    "default".to_string(),
                    "fast".to_string(),
                    "pro".to_string(),
                ];
                let model = pick_from_list(&models, "LLM7.io models", "llm7");
                match crate::llm7::Llm7Chat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Llm7(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "11" => {
                let key = get_or_prompt_key("kilo", "Enter your Kilo Code API key (from app.kilo.ai): ");
                let models = vec![
                    "kilo-auto/frontier".to_string(),
                    "kilo-auto/balanced".to_string(),
                    "kilo-auto/free".to_string(),
                    "kilo-auto/small".to_string(),
                    "anthropic/claude-opus-4.7".to_string(),
                    "anthropic/claude-sonnet-4.6".to_string(),
                    "anthropic/claude-haiku-4.5".to_string(),
                    "openai/gpt-5.4".to_string(),
                    "openai/gpt-5.4-mini".to_string(),
                    "google/gemini-3.1-pro-preview".to_string(),
                    "google/gemini-2.5-flash".to_string(),
                    "x-ai/grok-4".to_string(),
                    "deepseek/deepseek-v3.2".to_string(),
                ];
                let model = pick_from_list(&models, "Kilo Code models", "kilo");
                match crate::kilo::KiloChat::new(&key, &model) {
                    Ok(c) => return Some(Provider::Kilo(c)),
                    Err(e) => { eprintln!("Error: {e}"); continue; }
                }
            }
            "12" | "f" => {
                let all_models = [
                    ("openai/free", "openrouter"), ("openai/gpt-4o-mini", "openrouter"),
                    ("deepseek/deepseek-chat", "openrouter"), ("deepseek/deepseek-r1", "openrouter"),
                    ("google/gemini-3-flash", "openrouter"), ("google/gemini-3.1-flash-lite", "openrouter"),
                    ("cohere/command-r7b-12-2025", "openrouter"),
                    ("meta-llama/llama-3.3-70b-instruct", "openrouter"),
                    ("mistralai/mistral-small-3.1-24b", "openrouter"),
                    ("llama-3.3-70b-versatile", "groq"), ("llama-3.1-8b-instant", "groq"),
                    ("mixtral-8x7b-32768", "groq"), ("gemma2-9b-it", "groq"),
                    ("mistral-large-latest", "mistral"), ("mistral-medium-latest", "mistral"),
                    ("mistral-small-latest", "mistral"), ("ministral-3b-latest", "mistral"),
                    ("ministral-8b-latest", "mistral"),
                    ("gpt-oss-120b", "cerebras"), ("zai-glm-4.7", "cerebras"),
                    ("deepseek-r1-0528", "llm7"), ("deepseek-v3-0324", "llm7"),
                    ("mistral-small-3.1-24b", "llm7"), ("qwen2.5-coder-32b", "llm7"),
                    ("gpt-4o-mini", "llm7"), ("gemini-2.5-flash-lite", "llm7"),
                    ("default", "llm7"), ("fast", "llm7"), ("pro", "llm7"),
                    ("kilo-auto/free", "kilo"), ("kilo-auto/small", "kilo"),
                ];
                let mut p = persist().lock().unwrap();
                let mut merged: Vec<(String, String)> = all_models.iter()
                    .map(|(m, pr)| (m.to_string(), pr.to_string()))
                    .collect();
                let providers: Vec<String> = all_models.iter().map(|(_, pr)| pr.to_string())
                    .collect::<std::collections::HashSet<_>>().into_iter().collect();
                for pr in &providers {
                    for cm in p.get_custom_models(pr) {
                        if !merged.iter().any(|(m, p)| m == &cm && p == pr) {
                            merged.push((cm, pr.clone()));
                        }
                    }
                }
                let mut seeded: Vec<String> = Vec::new();
                for (_, prov) in &merged {
                    if !seeded.contains(prov) && p.get_blocked_models(prov).is_empty() {
                        seeded.push(prov.clone());
                        for (m, pr) in &merged {
                            if pr == prov { p.block_model(pr, m); }
                        }
                    }
                }
                let available: Vec<(String, String)> = merged.iter()
                    .filter(|(m, prov)| !p.is_model_blocked(prov, m))
                    .cloned()
                    .collect();
                drop(p);
                if available.is_empty() {
                    println!("No free models (all blocked).");
                    continue;
                }
                loop {
                    println!("\nOnly free models:");
                    for (i, (m, prov)) in available.iter().enumerate() {
                        println!("  {}. [{prov}] {m}", i + 1);
                    }
                    println!("  0. Go back");
                    print!("Select (1-{}, 0 to go back): ", available.len());
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();
                    let input = input.trim();
                    if input == "0" { break; }
                    if input.is_empty() { continue; }
                    if let Ok(n) = input.parse::<usize>() {
                        if n >= 1 && n <= available.len() {
                            let (model, prov) = &available[n - 1];
                            match prov.as_str() {
                                "openrouter" => {
                                    let key = get_or_prompt_key("openrouter", "Enter your OpenRouter API key: ");
                                    match crate::openrouter::OpenRouterChat::new(&key, model) {
                                        Ok(c) => return Some(Provider::OpenRouter(c)),
                                        Err(e) => { eprintln!("Error: {e}"); continue; }
                                    }
                                }
                                "groq" => {
                                    let key = get_or_prompt_key("groq", "Enter your Groq API key: ");
                                    match crate::groq::GroqChat::new(&key, model) {
                                        Ok(c) => return Some(Provider::Groq(c)),
                                        Err(e) => { eprintln!("Error: {e}"); continue; }
                                    }
                                }
                                "mistral" => {
                                    let key = get_or_prompt_key("mistral", "Enter your Mistral API key: ");
                                    match crate::mistral::MistralChat::new(&key, model) {
                                        Ok(c) => return Some(Provider::Mistral(c)),
                                        Err(e) => { eprintln!("Error: {e}"); continue; }
                                    }
                                }
                                "cerebras" => {
                                    let key = get_or_prompt_key("cerebras", "Enter your Cerebras API key: ");
                                    match crate::cerebras::CerebrasChat::new(&key, model) {
                                        Ok(c) => return Some(Provider::Cerebras(c)),
                                        Err(e) => { eprintln!("Error: {e}"); continue; }
                                    }
                                }
                                "llm7" => {
                                    let key = get_or_prompt_key("llm7", "Enter your LLM7.io token: ");
                                    match crate::llm7::Llm7Chat::new(&key, model) {
                                        Ok(c) => return Some(Provider::Llm7(c)),
                                        Err(e) => { eprintln!("Error: {e}"); continue; }
                                    }
                                }
                                "kilo" => {
                                    let key = get_or_prompt_key("kilo", "Enter your Kilo Code API key: ");
                                    match crate::kilo::KiloChat::new(&key, model) {
                                        Ok(c) => return Some(Provider::Kilo(c)),
                                        Err(e) => { eprintln!("Error: {e}"); continue; }
                                    }
                                }
                                _ => { eprintln!("Unknown provider: {prov}"); continue; }
                            }
                        }
                    }
                    println!("Invalid.");
                }
                continue;
            }
            _ => { println!("Invalid choice."); continue; }
        };
    }
}

pub fn choose_provider_boxed() -> Option<ProviderBox> {
    choose_provider().map(|p| Box::new(p) as ProviderBox)
}

pub fn prompt_anyway(pname: &str) -> bool {
    print!("\nProvider is on cooldown due to a previous error. Try anyway? (y/N): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    if input.trim().eq_ignore_ascii_case("y") {
        persist().lock().unwrap().mark_healthy(pname);
        true
    } else {
        println!("Skipped.");
        false
    }
}

pub fn handle_error(pname: &str, msg: &str) {
    if msg.contains("429 Too Many Requests") {
        println!("Rate limited. Will retry once...");
    } else if msg.contains("503") || msg.contains("502") || msg.contains("5") {
        persist().lock().unwrap().mark_error(pname, msg);
        eprintln!("Error (marked cooldown): {msg}");
    } else if msg.contains("401") || msg.contains("403") || msg.contains("unauthorized") || msg.contains("permission") {
        persist().lock().unwrap().rotate_key(pname);
        let new_key = persist().lock().unwrap().get_key(pname).map(|k| {
            if k.len() > 8 { format!("{}...{}", &k[..4], &k[k.len()-4..]) } else { k }
        }).unwrap_or_default();
        eprintln!("Auth error — rotated to next key ({new_key})");
    } else {
        eprintln!("Error: {msg}");
    }
}

pub fn handle_config(input: &str) {
    let args: Vec<&str> = input.split_whitespace().collect();
    match args.as_slice() {
        ["/config"] => {
            let p = persist().lock().unwrap();
            for (name, cfg) in &p.providers {
                let status = if cfg.healthy { "ok" } else { "cooldown" };
                let blocked = cfg.blocked_models.len();
                let (pt, ct) = p.get_token_usage(name);
                println!("\n{name}: {status} | keys: {} | blocked: {blocked} | tokens: {pt}in/{ct}out", cfg.keys.len());
            }
        }
        _ => println!("Usage: /config"),
    }
}
