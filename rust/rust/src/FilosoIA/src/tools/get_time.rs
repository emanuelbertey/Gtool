use std::collections::HashMap;
use async_trait::async_trait;
use chrono::Local;
use super::ToolParam;
use super::Tool;

pub struct GetTime;

#[async_trait]
impl Tool for GetTime {
    fn name(&self) -> &'static str { "get_time" }
    fn description(&self) -> &'static str { "Get the current date and time" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "format",
            param_type: "string",
            description: "Optional format: 'full', 'date', 'time', or 'unix' (default: 'full')",
            required: false,
        }]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let fmt = args.get("format").map(|s| s.as_str()).unwrap_or("full");
        let now = Local::now();
        match fmt {
            "date" => Ok(now.format("%Y-%m-%d").to_string()),
            "time" => Ok(now.format("%H:%M:%S").to_string()),
            "unix" => Ok(now.timestamp().to_string()),
            _ => Ok(now.format("%Y-%m-%d %H:%M:%S").to_string()),
        }
    }
}
