use std::collections::HashMap;
use async_trait::async_trait;
use super::ToolParam;
use super::Tool;

pub struct WriteFile;

fn sanitize(path: &str) -> String {
    path.replace("..", "").replace("~", "").replace("\\", "/")
}

#[async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &'static str { "write_file" }
    fn description(&self) -> &'static str { "Create/edit/read files. Modes: 'write' (overwrite), 'append', 'insert' (at line, pushes rest down), 'replace' (line), 'read' (lines)" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![
            ToolParam {
                name: "path",
                param_type: "string",
                description: "Relative path from project root (e.g. 'sum.py')",
                required: true,
            },
            ToolParam {
                name: "content",
                param_type: "string",
                description: "Content to write (ignored in 'read' mode)",
                required: false,
            },
            ToolParam {
                name: "mode",
                param_type: "string",
                description: "'write' (default), 'append', 'insert' (at line, pushes down), 'replace' (line), 'read' (show start-end)",
                required: false,
            },
            ToolParam {
                name: "start_line",
                param_type: "string",
                description: "Line number (1-based) for 'replace' or 'read' mode",
                required: false,
            },
            ToolParam {
                name: "end_line",
                param_type: "string",
                description: "End line number for 'read' mode (shows start_line to end_line)",
                required: false,
            },
        ]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let path = args.get("path").ok_or("Missing 'path' argument")?;
        let sanitized = sanitize(path);
        let canonical = std::path::Path::new(&sanitized);
        if canonical.is_absolute() {
            return Err("Absolute paths not allowed".into());
        }

        let mode = args.get("mode").map(|s| s.as_str()).unwrap_or("write");
        let start_line = args.get("start_line").and_then(|s| s.parse::<usize>().ok());
        let end_line = args.get("end_line").and_then(|s| s.parse::<usize>().ok());

        match mode {
            "read" => {
                let content = std::fs::read_to_string(&sanitized)
                    .map_err(|e| format!("Cannot read file: {e}"))?;
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let from = start_line.unwrap_or(1).max(1).min(total).saturating_sub(1);
                let to = end_line.unwrap_or(total).max(1).min(total);
                if from >= to || from >= total {
                    return Err(format!("File has {total} lines, requested {}-{}", from + 1, to));
                }
                let selected: Vec<&str> = lines[from..to].to_vec();
                let out = selected.iter().enumerate()
                    .map(|(i, l)| format!("{:>4}: {}", from + i + 1, l))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(format!("File '{path}' ({total} lines, showing {}-{}):\n{out}", from + 1, to))
            }
            "append" => {
                let content = args.get("content").ok_or("Missing 'content' for append mode")?;
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&sanitized)
                    .map_err(|e| format!("Cannot open file: {e}"))?;
                writeln!(file, "{content}").map_err(|e| format!("Cannot append: {e}"))?;
                Ok(format!("Appended to '{path}'"))
            }
            "replace" => {
                let content = args.get("content").ok_or("Missing 'content' for replace mode")?;
                let line_num = start_line.ok_or("Missing 'start_line' for replace mode")?;
                let existing = std::fs::read_to_string(&sanitized)
                    .map_err(|e| format!("Cannot read file: {e}"))?;
                let mut lines: Vec<&str> = existing.lines().collect();
                if line_num == 0 || line_num > lines.len() {
                    return Err(format!("File has {} lines, cannot replace line {}", lines.len(), line_num));
                }
                lines[line_num - 1] = content;
                let new_content = lines.join("\n");
                std::fs::write(&sanitized, &new_content)
                    .map_err(|e| format!("Cannot write file: {e}"))?;
                Ok(format!("Replaced line {line_num} in '{path}'"))
            }
            "insert" => {
                let content = args.get("content").ok_or("Missing 'content' for insert mode")?;
                let line_num = start_line.ok_or("Missing 'start_line' for insert mode")?;
                let existing = std::fs::read_to_string(&sanitized)
                    .map_err(|e| format!("Cannot read file: {e}"))?;
                let mut lines: Vec<String> = existing.lines().map(String::from).collect();
                if line_num == 0 || line_num > lines.len() + 1 {
                    return Err(format!("File has {} lines, cannot insert at line {}", lines.len(), line_num));
                }
                let insert_idx = line_num - 1;
                for new_line in content.lines().rev() {
                    lines.insert(insert_idx, new_line.to_string());
                }
                let new_content = lines.join("\n");
                std::fs::write(&sanitized, &new_content)
                    .map_err(|e| format!("Cannot write file: {e}"))?;
                Ok(format!("Inserted {} line(s) at line {line_num} in '{path}'", content.lines().count()))
            }
            _ => {
                let content = args.get("content").ok_or("Missing 'content' argument")?;
                std::fs::write(&sanitized, content)
                    .map_err(|e| format!("Cannot write file: {e}"))?;
                Ok(format!("File '{path}' written ({} bytes)", content.len()))
            }
        }
    }
}