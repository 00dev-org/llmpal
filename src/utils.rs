use crate::app::LlmpalError;
use std::fs;
use std::path::Path;

pub fn write_diagnostic_log(content: &str) -> Result<(), LlmpalError> {
    let home_dir = match std::env::var("HOME") {
        Ok(h) => Path::new(&h).to_path_buf(),
        Err(_) => return Ok(()),
    };

    let diag_dir = home_dir.join(".llmpal");

    if !diag_dir.exists() {
        fs::create_dir_all(&diag_dir).map_err(|e| {
            LlmpalError::FileError(format!("failed to create diagnostic directory: {}", e))
        })?;
    }

    let log_path = diag_dir.join("prompt.log");
    fs::write(&log_path, content)
        .map_err(|e| LlmpalError::FileError(format!("failed to write diagnostic log: {}", e)))
}

pub fn write_dump_log(content: &str) -> Result<String, String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("dump_{}.log", timestamp);
    fs::write(&filename, content).map_err(|e| format!("Failed to save dump log: {}", e))?;
    Ok(filename)
}
