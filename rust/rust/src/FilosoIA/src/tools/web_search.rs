use std::collections::HashMap;
use async_trait::async_trait;
use super::ToolParam;
use super::Tool;

pub struct WebSearch;

fn url_encode(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}

#[async_trait]
impl Tool for WebSearch {
    fn name(&self) -> &'static str { "web_search" }
    fn description(&self) -> &'static str { "Search the web for information using DuckDuckGo" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "query",
            param_type: "string",
            description: "The search query",
            required: true,
        }]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let query = args.get("query").ok_or("Missing 'query' argument")?;
        let encoded = url_encode(query);
        let url = format!("https://lite.duckduckgo.com/lite/?q={}", encoded);

        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!("try {{ $r = Invoke-RestMethod -Uri '{url}' -UseBasicParsing -TimeoutSec 10 -ErrorAction Stop; if ($r -is [string]) {{ $r }} else {{ $r.Content }} }} catch {{ write-error $_ }}")
            ])
            .output()
            .map_err(|e| format!("Failed to run search: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let result = if stdout.is_empty() { stderr } else { stdout };

        // Strip HTML tags for a readable summary
        let stripped = result.replace("<tr>", "\n").replace("</tr>", "")
            .replace("<td>", "").replace("</td>", " | ")
            .replace("<a ", "\n").replace("</a>", "")
            .replace("class=\"result-snippet\"", "")
            .replace("class=\"result-link\"", "")
            .replace("href=\"", "").replace("\">", ": ");

        let clean: String = stripped.chars()
            .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace() || c == '\n')
            .collect();

        let lines: Vec<&str> = clean.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.contains("<") && l.len() > 5)
            .collect();

        if lines.is_empty() {
            return Err(format!("No results found for '{query}'"));
        }

        let preview = lines.join("\n");
        if preview.len() > 4000 {
            Ok(format!("{}... (truncated)", &preview[..4000]))
        } else {
            Ok(preview)
        }
    }
}
