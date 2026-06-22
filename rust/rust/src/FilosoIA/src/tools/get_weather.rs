use std::collections::HashMap;
use async_trait::async_trait;
use super::ToolParam;
use super::Tool;

pub struct GetWeather;

fn url_encode(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}

#[async_trait]
impl Tool for GetWeather {
    fn name(&self) -> &'static str { "get_weather" }
    fn description(&self) -> &'static str { "Get current weather for a location (uses wttr.in)" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "location",
            param_type: "string",
            description: "City name or location (e.g. 'London', 'New York')",
            required: true,
        }]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let location = args.get("location").ok_or("Missing 'location' argument")?;
        let encoded = url_encode(location);
        let url = format!("https://wttr.in/{}?format=%t+%c+%w+%h&lang=es", encoded);

        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!("try {{ (Invoke-RestMethod -Uri '{url}' -UseBasicParsing -TimeoutSec 10 -ErrorAction Stop).trim() }} catch {{ write-error $_ }}")
            ])
            .output()
            .map_err(|e| format!("Failed to get weather: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string().trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !stdout.is_empty() {
            Ok(format!("Weather in {location}: {stdout}"))
        } else if !stderr.is_empty() {
            Err(format!("Weather error: {stderr}"))
        } else {
            Err(format!("Could not get weather for '{location}'"))
        }
    }
}