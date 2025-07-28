use crate::app::LlmpalError;
use std::fs;
use std::io::Write;
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

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| LlmpalError::FileError(format!("failed to open diagnostic log: {}", e)))?
        .write_all(content.as_bytes())
        .map_err(|e| LlmpalError::FileError(format!("failed to write diagnostic log: {}", e)))?;

    Ok(())
}
