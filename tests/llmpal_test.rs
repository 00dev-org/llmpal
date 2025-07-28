#[cfg(test)]
mod tests {
    use llmpal::app::run;
    use llmpal::config::Cli;
    use mockito::Mock;
    use std::error::Error;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_run_method() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let test_file_path = temp_dir.path().join("test.txt");
        fs::write(&test_file_path, "test content")?;

        let mut server = mockito::Server::new_async().await;
        let api_url = server.url();

        let config_path = temp_dir.path().join(".llmpal.json");
        let config_content = format!(
            r#"{{
                "models": [{{
                    "code": "test-model",
                    "model": "test-model",
                    "prompt_cost": 0.001,
                    "completion_cost": 0.001,
                    "api_url": "{}",
                    "api_key": "test-key"
                }}]
            }}"#,
            api_url
        );
        fs::write(&config_path, config_content)?;

        let old_cwd = std::env::current_dir()?;
        std::env::set_current_dir(temp_dir.path())?;

        let mock_response = serde_json::json!({
            "choices": [{
                "message": {
                    "content": format!("=== EXPLAIN START ===\nTest explanation\n=== EXPLAIN END ===\n=== {} === START ===\nmodified content\n=== {} === END ===",
                        &test_file_path.to_string_lossy(),
                        &test_file_path.to_string_lossy(),
                    )
                }
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        });

        let _mock: Mock = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create_async()
            .await;

        let args = Cli {
            instruction: "Test instruction".to_string(),
            files: vec![test_file_path.to_str().unwrap().to_string()],
            model: None,
            output: None,
            verbose: false,
            trace: false,
        };

        let result = run(&args).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&test_file_path)?;
        assert_eq!(content, "modified content");

        std::env::set_current_dir(old_cwd)?;
        Ok(())
    }
}
