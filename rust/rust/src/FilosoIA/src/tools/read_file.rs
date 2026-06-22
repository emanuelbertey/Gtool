use std::collections::HashMap;
use async_trait::async_trait;
use super::ToolParam;
use super::Tool;

pub struct ReadFile;

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &'static str { "read_file" }
    fn description(&self) -> &'static str { "Read the contents of a local file (only allowed in project directory)" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "path",
            param_type: "string",
            description: "Relative path from the project root (e.g. 'src/main.rs')",
            required: true,
        }]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let path = args.get("path").ok_or("Missing 'path' argument")?;
        let sanitized = path.replace("..", "").replace("~", "").replace("\\", "/");
        let canonical = std::path::Path::new(&sanitized);
        if canonical.is_absolute() {
            return Err("Absolute paths not allowed".into());
        }
        match std::fs::read_to_string(&sanitized) {
            Ok(content) => Ok(content),
            Err(e) => Err(format!("Cannot read file: {e}")),
        }
    }
}
