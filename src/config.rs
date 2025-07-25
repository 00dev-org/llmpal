use clap::Parser;
use std::fs;

pub const OPEN_ROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
pub const DEFAULT_MODEL: &str = "moonshotai/kimi-k2";
pub const DEFAULT_PROMPT_COST: f64 = 0.60;
pub const DEFAULT_COMPLETION_COST: f64 = 2.50;
pub const DEFAULT_MAX_TOKENS: usize = 16384;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, after_help = "\
Examples:\n\
  llmpal -f src/main.rs 'Generate unit tests'\n\
  llmpal -f src/main.rs -f src/config.rs -o README.md 'Create a README.md file'\n\
  llmpal -f src/main.rs 'Explain what this code is doing'\n\
  llmpal -o src/countries.json 'Create a JSON file with a list of G20 countries. Fields: name, code.'\n\
  ")]
pub struct Cli {
    #[arg(
        long = "file",
        short = 'f',
        value_name = "FILE",
        help = "Input files to work with. They will be sent to the LLM, and might be modified."
    )]
    pub files: Vec<String>,
    #[arg(long, short = 'v', help = "Logs LLM prompt and response to stderr.")]
    pub verbose: bool,
    #[arg(
        long,
        short = 'o',
        value_name = "OUTPUT",
        help = "Path to output file. The LLM will be allowed to write to it."
    )]
    pub output: Option<String>,
    #[arg(
        long,
        help = "Logs the full JSON sent and received during API calls to stderr."
    )]
    pub trace: bool,
    #[arg(
        long,
        short = 'm',
        value_name = "MODEL",
        help = "Use a different model configured in the .llmpal.json file."
    )]
    pub model: Option<String>,
    #[arg(value_name = "INSTRUCTIONS", help = "Instructions for the LLM.")]
    pub instruction: String,
}

#[derive(serde::Deserialize)]
pub struct ModelConfig {
    pub code: String,
    pub model: String,
    pub prompt_cost: f64,
    pub completion_cost: f64,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub max_tokens: Option<usize>,
    pub provider: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct Config {
    pub models: Option<Vec<ModelConfig>>,
    pub rules: Option<Vec<String>>,
}

fn config_from_path<P: AsRef<std::path::Path>>(path: P) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or(Config {
            models: None,
            rules: None,
        })
}

pub fn get_config() -> Config {
    let local_path = std::path::Path::new(".llmpal.json");
    if local_path.exists() {
        return config_from_path(local_path);
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = std::path::PathBuf::from(home).join(".llmpal.json");
        return config_from_path(home_path);
    }

    Config {
        models: None,
        rules: None,
    }
}

fn get_selected_model_code(args: &Cli, config: &Config) -> String {
    args.model
        .clone()
        .or(config
            .models
            .as_ref()
            .and_then(|models| models.first().map(|m| m.code.clone())))
        .unwrap_or(DEFAULT_MODEL.to_string())
}

fn resolve_env_token(token: &str) -> String {
    if token.starts_with('$') {
        let env_var = &token[1..];
        std::env::var(env_var).unwrap_or_else(|_| token.to_string())
    } else {
        token.to_string()
    }
}

pub fn get_model_config(args: &Cli, config: &Config) -> ModelConfig {
    let selected_model_code = get_selected_model_code(args, config);

    let model_config = config
        .models
        .as_ref()
        .and_then(|models| models.iter().find(|m| m.code == selected_model_code));

    ModelConfig {
        code: selected_model_code,
        model: model_config
            .map(|m| m.model.clone())
            .unwrap_or(DEFAULT_MODEL.to_string()),
        prompt_cost: model_config
            .map(|m| m.prompt_cost)
            .unwrap_or(DEFAULT_PROMPT_COST),
        completion_cost: model_config
            .map(|m| m.completion_cost)
            .unwrap_or(DEFAULT_COMPLETION_COST),
        api_url: model_config
            .as_ref()
            .and_then(|m| m.api_url.clone())
            .clone(),
        api_key: model_config
            .as_ref()
            .and_then(|m| m.api_key.as_ref().map(|token| resolve_env_token(token))),
        max_tokens: model_config.as_ref().and_then(|m| m.max_tokens),
        provider: model_config.as_ref().and_then(|m| m.provider.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[cfg(test)]
    mod cli_parsing {
        use super::*;

        #[test]
        fn test_cli_parsing() {
            let cli = Cli::parse_from([
                "llmpal",
                "-f",
                "test.txt",
                "-o",
                "out.txt",
                "-v",
                "--trace",
                "-m",
                "test-model",
                "test instruction",
            ]);
            assert_eq!(cli.files, vec!["test.txt"]);
            assert_eq!(cli.instruction, "test instruction");
            assert!(cli.verbose);
            assert!(cli.trace);
            assert_eq!(cli.output, Some("out.txt".to_string()));
            assert_eq!(cli.model, Some("test-model".to_string()));
        }
    }

    #[cfg(test)]
    mod model_config {
        use super::*;

        #[test]
        fn test_default_model_config() {
            let args = Cli::parse_from(["llmpal", "instruction"]);
            let config = Config {
                models: None,
                rules: None,
            };
            let model_config = get_model_config(&args, &config);
            assert_eq!(model_config.model, DEFAULT_MODEL);
            assert_eq!(model_config.prompt_cost, DEFAULT_PROMPT_COST);
            assert_eq!(model_config.completion_cost, DEFAULT_COMPLETION_COST);
            assert_eq!(model_config.code, DEFAULT_MODEL);
        }

        #[test]
        fn test_model_config_resolution() {
            let config = Config {
                models: Some(vec![ModelConfig {
                    code: "kimi".to_string(),
                    model: "test-model".to_string(),
                    prompt_cost: 1.1,
                    completion_cost: 2.2,
                    api_url: None,
                    api_key: Some("$TOKEN".to_string()),
                    max_tokens: Some(4096),
                    provider: Some("fireworks".to_string()),
                }]),
                rules: None,
            };

            let args = Cli::parse_from(["llmpal", "instruction", "--model", "kimi"]);
            let model_config = get_model_config(&args, &config);

            assert_eq!(model_config.model, "test-model");
            assert_eq!(model_config.prompt_cost, 1.1);
            assert_eq!(model_config.completion_cost, 2.2);
            assert_eq!(model_config.max_tokens, Some(4096));
            assert_eq!(model_config.code, "kimi");
            assert_eq!(model_config.api_key.as_deref(), Some("$TOKEN"));
            assert_eq!(model_config.provider, Some("fireworks".to_string()));
        }

        #[test]
        fn test_fallback_to_first_model_when_no_model_specified() {
            let config = Config {
                models: Some(vec![ModelConfig {
                    code: "other".to_string(),
                    model: "other-model".to_string(),
                    prompt_cost: 0.5,
                    completion_cost: 1.0,
                    api_url: None,
                    api_key: None,
                    max_tokens: None,
                    provider: None,
                }]),
                rules: None,
            };

            let args = Cli::parse_from(["llmpal", "instruction"]);
            let model_config = get_model_config(&args, &config);

            assert_eq!(model_config.model, "other-model");
            assert_eq!(model_config.prompt_cost, 0.5);
            assert_eq!(model_config.completion_cost, 1.0);
            assert_eq!(model_config.max_tokens, None);
            assert_eq!(model_config.provider, None);
        }

        #[test]
        fn test_specified_model_not_in_config() {
            let config = Config {
                models: Some(vec![]),
                rules: None,
            };
            let args = Cli::parse_from(["llmpal", "--model", "missing", "instruction"]);
            let model_config = get_model_config(&args, &config);
            assert_eq!(model_config.code, "missing");
            assert_eq!(model_config.model, DEFAULT_MODEL);
            assert_eq!(model_config.prompt_cost, DEFAULT_PROMPT_COST);
            assert_eq!(model_config.completion_cost, DEFAULT_COMPLETION_COST);
            assert_eq!(model_config.provider, None);
        }
    }

    #[cfg(test)]
    mod env_tokens {
        use super::*;

        #[test]
        fn test_resolve_env_token_with_var() {
            unsafe {
                env::set_var("TEST_TOKEN", "actual_value");
            }
            assert_eq!(resolve_env_token("$TEST_TOKEN"), "actual_value");
            unsafe {
                env::remove_var("TEST_TOKEN");
            }
        }

        #[test]
        fn test_resolve_env_token_missing_var() {
            assert_eq!(resolve_env_token("$MISSING_VAR"), "$MISSING_VAR");
        }

        #[test]
        fn test_resolve_env_token_plain_string() {
            assert_eq!(resolve_env_token("plain_val"), "plain_val");
        }
    }

    #[cfg(test)]
    mod config_loading {
        use super::*;

        #[test]
        fn test_config_from_invalid_json() {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("config.json");
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "{{ invalid json!").unwrap();
            let config = config_from_path(dir.path().join("config.json"));
            assert!(config.models.is_none());
            assert!(config.rules.is_none());
        }

        #[test]
        fn test_config_from_missing_file() {
            let config = config_from_path("nonexistent.json");
            assert!(config.models.is_none());
            assert!(config.rules.is_none());
        }
    }

    #[cfg(test)]
    mod config_precedence {
        use super::*;

        #[test]
        fn test_get_config_uses_local_file() {
            let temp_dir = tempdir().unwrap();
            let local_config_path = temp_dir.path().join(".llmpal.json");
            let config_content = "{\n  \"models\": [{\n    \"code\": \"local_code\",\n    \"model\": \"local_model\",\n    \"prompt_cost\": 1.5,\n    \"completion_cost\": 2.5\n  }],\n  \"rules\": [\"rule1\"]\n}";
            fs::write(&local_config_path, config_content).unwrap();

            let old_cwd = env::current_dir().unwrap();
            env::set_current_dir(temp_dir.path()).unwrap();

            let config = get_config();

            env::set_current_dir(old_cwd).unwrap();

            let vec = config.models.unwrap();
            let model_config = vec.get(0).unwrap();
            assert_eq!(model_config.code, "local_code");
            assert_eq!(model_config.model, "local_model");
            assert_eq!(model_config.prompt_cost, 1.5);
            assert_eq!(model_config.completion_cost, 2.5);
            assert_eq!(config.rules.as_ref().unwrap()[0], "rule1");
        }

        #[test]
        fn test_get_config_uses_home_file() {
            let home_dir = tempdir().unwrap();
            let home_config_path = home_dir.path().join(".llmpal.json");
            let config_content = "{\n  \"models\": [{\n    \"code\": \"home_code\",\n    \"model\": \"home_model\",\n    \"prompt_cost\": 1.0,\n    \"completion_cost\": 2.0\n  }],\n  \"rules\": [\"rule2\"]\n}";
            fs::write(&home_config_path, config_content).unwrap();

            let work_dir = tempdir().unwrap();
            env::set_current_dir(work_dir.path()).unwrap();

            let old_home = env::var_os("HOME");
            unsafe {
                env::set_var("HOME", home_dir.path().to_str().unwrap());
            }

            let config = get_config();

            if let Some(old_home) = old_home {
                unsafe {
                    env::set_var("HOME", old_home);
                }
            } else {
                unsafe {
                    env::remove_var("HOME");
                }
            }

            let vec = config.models.unwrap();
            let model_config = vec.get(0).unwrap();
            assert_eq!(model_config.code, "home_code");
            assert_eq!(model_config.model, "home_model");
            assert_eq!(model_config.prompt_cost, 1.0);
            assert_eq!(model_config.completion_cost, 2.0);
            assert_eq!(config.rules.as_ref().unwrap()[0], "rule2");
        }
    }
}
