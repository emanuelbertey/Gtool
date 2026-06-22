use crate::{persist, ProviderBox, tools};
use edgequake_llm::{ChatMessage, ToolDefinition};
use tokio::sync::mpsc;

pub enum ChatEvent {
    Response {
        sid: usize,
        content: String,
        prompt_tokens: u64,
        completion_tokens: u64,
        pname: String,
        provider: Option<ProviderBox>,
    },
    Error {
        sid: usize,
        error: String,
        pname: String,
        provider: Option<ProviderBox>,
    },
}

pub fn trim_buffer(msgs: &mut Vec<ChatMessage>) {
    const MAX_MSGS: usize = 50;
    while msgs.len() > MAX_MSGS {
        msgs.remove(0);
    }
}

pub fn handle_error_spawn(pname: &str, msg: &str) {
    if msg.contains("429 Too Many Requests") {
        println!("Rate limited.");
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

pub fn spawn_chat_task(
    tx: mpsc::UnboundedSender<ChatEvent>,
    sid: usize,
    pname: &str,
    provider: ProviderBox,
    msgs: Vec<ChatMessage>,
    tool_defs: Vec<ToolDefinition>,
) {
    let tx_clone = tx.clone();
    let pname = pname.to_string();
    let has_tools = provider.supports_function_calling() && !tool_defs.is_empty();
    tokio::spawn(async move {
        if has_tools {
            let result = provider.chat_with_tools(&msgs, &tool_defs).await;
            let result = result.map_err(|e| e.to_string());
            match result {
                Ok(response) => {
                    let mut msgs = msgs;
                    if !response.tool_calls.is_empty() {
                        msgs.push(ChatMessage::assistant(&response.content));
                        let all_tools = tools::all_tools();
                        for tc in &response.tool_calls {
                            if let Some(tool) = tools::find_tool(&all_tools, &tc.function.name) {
                                let args: std::collections::HashMap<String, String> =
                                    serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                                match tool.call(args).await {
                                    Ok(r) => msgs.push(ChatMessage::tool_result(tc.id.clone(), r)),
                                    Err(e) => msgs.push(ChatMessage::tool_result(tc.id.clone(), format!("Error: {e}"))),
                                }
                            }
                        }
                        match provider.chat(&msgs).await {
                            Ok(follow) => {
                                tx_clone.send(ChatEvent::Response {
                                    sid, content: follow.content,
                                    prompt_tokens: (response.prompt_tokens + follow.prompt_tokens) as u64,
                                    completion_tokens: (response.completion_tokens + follow.completion_tokens) as u64,
                                    pname, provider: Some(provider),
                                }).ok();
                            }
                            Err(e) => {
                                handle_error_spawn(&pname, &e.to_string());
                                tx_clone.send(ChatEvent::Error { sid, error: e.to_string(), pname, provider: Some(provider) }).ok();
                            }
                        }
                    } else {
                        tx_clone.send(ChatEvent::Response {
                            sid, content: response.content,
                            prompt_tokens: response.prompt_tokens as u64,
                            completion_tokens: response.completion_tokens as u64,
                            pname, provider: Some(provider),
                        }).ok();
                    }
                }
                Err(msg) => {
                    if msg.contains("429 Too Many Requests") {
                        println!("Rate limited. Retrying once...");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        match provider.chat_with_tools(&msgs, &tool_defs).await {
                            Ok(r) => {
                                tx_clone.send(ChatEvent::Response {
                                    sid, content: r.content,
                                    prompt_tokens: r.prompt_tokens as u64,
                                    completion_tokens: r.completion_tokens as u64, pname, provider: Some(provider),
                                }).ok();
                            }
                            Err(e2) => {
                                handle_error_spawn(&pname, &e2.to_string());
                                tx_clone.send(ChatEvent::Error { sid, error: e2.to_string(), pname, provider: Some(provider) }).ok();
                            }
                        }
                    } else {
                        handle_error_spawn(&pname, &msg);
                        tx_clone.send(ChatEvent::Error { sid, error: msg, pname, provider: Some(provider) }).ok();
                    }
                }
            }
        } else {
            let result = provider.chat(&msgs).await;
            let result = result.map_err(|e| e.to_string());
            match result {
                Ok(response) => {
                    tx_clone.send(ChatEvent::Response {
                        sid, content: response.content,
                        prompt_tokens: response.prompt_tokens as u64,
                        completion_tokens: response.completion_tokens as u64,
                        pname, provider: Some(provider),
                    }).ok();
                }
                Err(msg) => {
                    if msg.contains("429 Too Many Requests") {
                        println!("Rate limited. Retrying once...");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        match provider.chat(&msgs).await {
                            Ok(r) => {
                                tx_clone.send(ChatEvent::Response {
                                    sid, content: r.content,
                                    prompt_tokens: r.prompt_tokens as u64,
                                    completion_tokens: r.completion_tokens as u64, pname, provider: Some(provider),
                                }).ok();
                            }
                            Err(e2) => {
                                handle_error_spawn(&pname, &e2.to_string());
                                tx_clone.send(ChatEvent::Error { sid, error: e2.to_string(), pname, provider: Some(provider) }).ok();
                            }
                        }
                    } else {
                        handle_error_spawn(&pname, &msg);
                        tx_clone.send(ChatEvent::Error { sid, error: msg, pname, provider: Some(provider) }).ok();
                    }
                }
            }
        }
    });
}
