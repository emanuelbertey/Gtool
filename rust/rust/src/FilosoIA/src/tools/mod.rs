use std::collections::HashMap;
use serde_json::json;

pub mod calculator;
pub mod get_time;
pub mod get_weather;
pub mod read_file;
pub mod run_command;
pub mod web_search;
pub mod write_file;

use async_trait::async_trait;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Vec<ToolParam>;
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String>;
}

pub struct ToolParam {
    pub name: &'static str,
    pub param_type: &'static str,
    pub description: &'static str,
    pub required: bool,
}

pub fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(calculator::Calculator),
        Box::new(get_time::GetTime),
        Box::new(get_weather::GetWeather),
        Box::new(web_search::WebSearch),
        Box::new(read_file::ReadFile),
        Box::new(run_command::RunCommand),
        Box::new(write_file::WriteFile),
    ]
}

pub fn to_tool_definitions(tools: &[Box<dyn Tool>]) -> Vec<edgequake_llm::ToolDefinition> {
    tools.iter().map(|t| {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for p in t.parameters() {
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), json!(p.param_type));
            prop.insert("description".to_string(), json!(p.description));
            properties.insert(p.name.to_string(), json!(prop));
            if p.required { required.push(p.name.to_string()); }
        }
        let schema = json!({
            "type": "object", "properties": properties,
            "required": required, "additionalProperties": false
        });
        edgequake_llm::ToolDefinition::function(t.name(), t.description(), schema)
    }).collect()
}

pub fn find_tool<'a>(tools: &'a [Box<dyn Tool>], name: &str) -> Option<&'a Box<dyn Tool>> {
    tools.iter().find(|t| t.name() == name)
}
