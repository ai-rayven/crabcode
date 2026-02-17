use std::fs;
use regex::Regex;
use lazy_static::lazy_static;
use crate::types::ToolCall;

lazy_static! {
    static ref READ_FILE_RE: Regex = Regex::new(r"<read_file>(.*?)</read_file>").unwrap();
}

pub fn parse_tool_call(content: &str) -> ToolCall {
    if let Some(caps) = READ_FILE_RE.captures(content) {
        let path = caps[1].to_string(); // caps[1] = content inside the tags
        return ToolCall::ReadFile(path);
    }
    ToolCall::None
}

pub fn execute_read(path: &str) -> String {
    if path.contains("..") || path.starts_with("/") {
        return "Error: Access denied. You can only read files in the current directory.".to_string();
    }

    // TODO: I think we should consider letting the agent look at a window of the file 
    // to handle really big files or some other mechanism for context mgmt.
    match fs::read_to_string(path) {
        Ok(content) => format!("File '{}' content: \n\n{}\n", path, content),
        Err(e) => format!("Error reading file '{}': {}", path, e),
    }
}