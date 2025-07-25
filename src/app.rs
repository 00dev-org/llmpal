use reqwest;
use serde_json;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use crate::{config, llm, spinner};

#[derive(Debug)]
pub enum LlmpalError {
    ApiKeyMissing,
    SerializeError(String),
    NetworkError(String),
    ParseError(String),
    FileError(String),
}

impl std::fmt::Display for LlmpalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmpalError::ApiKeyMissing => write!(f, "Missing OPENROUTER_API_KEY env variable"),
            LlmpalError::SerializeError(e) => write!(f, "Failed to serialize JSON: {}", e),
            LlmpalError::NetworkError(e) => write!(f, "{}", e),
            LlmpalError::ParseError(e) => write!(f, "{}", e),
            LlmpalError::FileError(e) => write!(f, "{}", e),
        }
    }
}

impl Error for LlmpalError {}

pub async fn run(args: &config::Cli) -> Result<(), LlmpalError> {
    let config = config::get_config();
    let rules = config.rules.clone().unwrap_or_default();

    let model_config = config::get_model_config(args, &config);

    let mut allowed_files_set: HashSet<String> = HashSet::new();
    let mut input_files: Vec<String> = Vec::new();

    for file in &args.files {
        let path = std::path::Path::new(file);
        if path.is_dir() {
            let entries = std::fs::read_dir(path)
                .map_err(|e| LlmpalError::FileError(format!("Cannot read directory '{}': {}", file, e)))?;
            for entry in entries {
                let entry = entry
                    .map_err(|e| LlmpalError::FileError(format!("Error reading entry in '{}': {}", file, e)))?;
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    continue;
                }
                if let Some(entry_str) = entry_path.as_os_str().to_str() {
                    allowed_files_set.insert(entry_str.to_string());
                    input_files.push(entry_str.to_string());
                }
            }
        } else {
            allowed_files_set.insert(file.clone());
            input_files.push(file.clone());
        }
    }

    if let Some(output) = &args.output {
        allowed_files_set.insert(output.clone());
    }
    let allowed_files: Vec<String> = allowed_files_set.into_iter().collect();

    let api_key = model_config
        .api_key
        .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        .ok_or(LlmpalError::ApiKeyMissing)?;

    let system_prompt = llm::build_system_prompt(&allowed_files, &rules);
    let user_prompt = llm::build_user_prompt(&args.instruction, &input_files, &args.output);

    let body = build_request(
        &model_config.model,
        model_config.provider.as_deref(),
        &system_prompt,
        &user_prompt,
        model_config
            .max_tokens
            .unwrap_or(config::DEFAULT_MAX_TOKENS),
        model_config.api_url.is_none(),
    ).map_err(|e| LlmpalError::SerializeError(e.to_string()))?;

    if args.trace {
        eprintln!("::DEBUG:: === RAW LLM REQUEST ===");
        eprintln!(
            "::DEBUG:: {}",
            serde_json::to_string_pretty(
                &serde_json::from_str::<serde_json::Value>(&body).unwrap()
            )
            .unwrap()
        );
    }
    if args.verbose {
        eprintln!("::DEBUG:: === SYSTEM PROMPT ===");
        eprintln!("::DEBUG:: {}", system_prompt);
        eprintln!("::DEBUG:: === USER PROMPT ===");
        eprintln!("::DEBUG:: {}", user_prompt);
    }

    let api_url = model_config
        .api_url
        .clone()
        .unwrap_or_else(|| config::OPEN_ROUTER_URL.to_string());

    let estimated_input_tokens = estimate_token_count(&system_prompt) + estimate_token_count(&user_prompt);

    let log_output = if let Some(provider) = &model_config.provider {
        format!(
            "Model: {} [provider: {}] | URL: {} | Cost: ${:.4}/1M prompt, ${:.4}/1M completion | Estimated input tokens: {}",
            model_config.model,
            provider,
            api_url,
            model_config.prompt_cost,
            model_config.completion_cost,
            estimated_input_tokens
        )
    } else {
        format!(
            "Model: {} | URL: {} | Cost: ${:.4}/1M prompt, ${:.4}/1M completion | Estimated input tokens: {}",
            model_config.model, api_url, model_config.prompt_cost, model_config.completion_cost, estimated_input_tokens
        )
    };

    eprintln!("{}", log_output);
    let start_time = Instant::now();

    let loading = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let spinner_handle = spinner::setup_spinner(loading.clone(), Some("Waiting for LLM response"));

    let res = send_api_request(&api_key, &api_url, &body)
        .await
        .map_err(|e| LlmpalError::NetworkError(e))?;

    let duration = start_time.elapsed();
    loading.store(false, std::sync::atomic::Ordering::Relaxed);
    spinner_handle.join().unwrap();

    if args.trace {
        eprintln!("::DEBUG:: === RAW LLM RESPONSE ===");
        eprintln!("::DEBUG:: {}", serde_json::to_string_pretty(&res).unwrap());
    }

    let resp_text = res["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| LlmpalError::ParseError("Invalid response format from API".to_string()))?
        .to_string();

    if args.verbose {
        eprintln!("::DEBUG:: === RAW LLM OUTPUT ===");
        eprintln!("::DEBUG:: {}", resp_text);
    }

    let loading_parse = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let spinner_parse_handle = spinner::setup_spinner(loading_parse.clone(), Some("Analyzing LLM response"));

    let (comments, files, _) = llm::parse_llm_response(&resp_text)
        .map_err(|e| LlmpalError::ParseError(e))?;

    for (path, _) in &files {
        if !allowed_files.contains(path) {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let filename = format!("dump_{}.log", timestamp);
            if let Err(e) = fs::write(&filename, &resp_text) {
                eprintln!("Failed to save dump: {}", e);
            }
            loading_parse.store(false, std::sync::atomic::Ordering::Relaxed);
            spinner_parse_handle.join().unwrap();
            return Err(LlmpalError::FileError(format!("attempting to write to disallowed file: {}", path)));
        }
    }

    loading_parse.store(false, std::sync::atomic::Ordering::Relaxed);
    spinner_parse_handle.join().unwrap();

    if !comments.is_empty() {
        println!("{}", comments);
    }

    for (path, content) in files.iter() {
        fs::write(path, content).map_err(|e| {
            LlmpalError::FileError(format!("writing file '{}': {}", path, e))
        })?;
    }

    let usage = &res["usage"];
    let provider_response = res.get("provider").and_then(|p| p.as_str());
    if let Some(prompt_tokens) = usage["prompt_tokens"].as_u64() {
        if let Some(completion_tokens) = usage["completion_tokens"].as_u64() {
            let prompt_cost_val = prompt_tokens as f64 * model_config.prompt_cost / 1_000_000.0;
            let completion_cost_val =
                completion_tokens as f64 * model_config.completion_cost / 1_000_000.0;
            let total_cost = prompt_cost_val + completion_cost_val;
            let tokens_per_second =
                (prompt_tokens + completion_tokens) as f64 / duration.as_secs_f64();
            let model_string = if let Some(provider_name) = provider_response {
                format!("{} [provider: {}]", model_config.model, provider_name)
            } else {
                model_config.model.clone()
            };
            eprintln!(
                "Model: {} | Prompt tokens: {} (${:.4}) | Completion tokens: {} (${:.4}) | Total tokens: {} (${:.4}) | Time: {:.2}s | Speed: {:.2} tokens/s",
                model_string,
                prompt_tokens,
                prompt_cost_val,
                completion_tokens,
                completion_cost_val,
                prompt_tokens + completion_tokens,
                total_cost,
                duration.as_secs_f64(),
                tokens_per_second
            );

            let max_tokens_allowed = model_config
                .max_tokens
                .unwrap_or(config::DEFAULT_MAX_TOKENS) as u64;
            if completion_tokens >= max_tokens_allowed {
                eprintln!(
                    "Warning: Completion tokens ({}) equal or exceed max token limit ({}). Output might be missing or incomplete.",
                    completion_tokens, max_tokens_allowed
                );
            }
        }
    }

    Ok(())
}

pub async fn send_api_request(
    api_key: &str,
    api_url: &str,
    body: &str,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();

    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/00dev-org/llmpal")
        .header("X-Title", "llmpal")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let error_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read error response: {}", e))?;
        return Err(format!(
            "API request failed with status {}: {}",
            status_code, error_text
        ));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {}", e))
}

pub fn build_request(
    model: &str,
    provider: Option<&str>,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: usize,
    is_default_api_url: bool,
) -> Result<String, Box<dyn Error>> {
    let mut body = serde_json::Map::new();

    body.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    body.insert(
        "max_tokens".to_string(),
        serde_json::Value::Number(max_tokens.into()),
    );
    body.insert(
        "messages".to_string(),
        serde_json::Value::Array(vec![
            serde_json::json!({
                "role": "system",
                "content": system_prompt
            }),
            serde_json::json!({
                "role": "user",
                "content": user_prompt
            }),
        ]),
    );

    let mut provider_obj: Option<serde_json::Map<String, serde_json::Value>> = None;

    if let Some(provider_name) = provider {
        let mut p = serde_json::Map::new();
        p.insert(
            "only".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String(provider_name.to_string())]),
        );
        provider_obj = Some(p);
    }

    if is_default_api_url {
        if provider_obj.is_none() {
            provider_obj = Some(serde_json::Map::new());
        }
        let p = provider_obj.as_mut().unwrap();
        p.insert(
            "data_collection".to_string(),
            serde_json::Value::String("deny".to_string()),
        );
    }

    if let Some(provider_obj) = provider_obj {
        body.insert(
            "provider".to_string(),
            serde_json::Value::Object(provider_obj),
        );
    }

    let json_value = serde_json::Value::Object(body);
    Ok(serde_json::to_string(&json_value)?)
}

fn estimate_token_count(text: &str) -> usize {
    text.chars().count() / 4
}