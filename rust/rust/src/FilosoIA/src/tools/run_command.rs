use std::collections::HashMap;
use async_trait::async_trait;
use super::ToolParam;
use super::Tool;

pub struct RunCommand;

#[async_trait]
impl Tool for RunCommand {
    fn name(&self) -> &'static str { "run_command" }
    fn description(&self) -> &'static str { "Run a shell command (sandboxed - read-only commands only)" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![
            ToolParam {
                name: "command",
                param_type: "string",
                description: "The command to run",
                required: true,
            },
        ]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let cmd = args.get("command").ok_or("Missing 'command' argument")?;
        if cmd.contains("rm ") || cmd.contains("del ") || cmd.contains("format ") || cmd.contains("mkfs ") {
            return Err("Destructive commands are not allowed".into());
        }
        let output = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .output()
            .map_err(|e| format!("Failed to run command: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let result = if stdout.is_empty() { stderr } else { stdout };
        if result.len() > 4000 {
            Ok(format!("{}... (truncated)", &result[..4000]))
        } else {
            Ok(result)
        }
    }
}
